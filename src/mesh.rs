use eframe::egui::{Pos2, Vec2};

#[derive(Default)]
pub struct Mesh {
	pub vertices: Vec<Pos2>,
	pub edges: Vec<[usize; 2]>,
}

impl Mesh {
	pub fn add_vertex(&mut self, pos: Pos2) -> usize {
		self.vertices.push(pos);
		self.vertices.len() - 1
	}

	pub fn add_edge(&mut self, a: usize, b: usize) {
		if a == b
			|| self
				.edges
				.iter()
				.any(|&edge| edge == [a, b] || edge == [b, a])
		{
			return;
		}

		self.edges.push([a, b]);
	}

	pub fn nearest_vertex(&self, pos: Pos2, radius: f32) -> Option<usize> {
		self.vertices
			.iter()
			.enumerate()
			.map(|(index, vertex)| (index, vertex.distance_sq(pos)))
			.filter(|(_, distance)| *distance <= radius * radius)
			.min_by(|a, b| a.1.total_cmp(&b.1))
			.map(|(index, _)| index)
	}

	pub fn remove_vertices(&mut self, mut indices: Vec<usize>) {
		indices.sort_unstable();
		for index in indices.into_iter().rev() {
			self.remove_vertex(index);
		}
	}

	pub fn triangles(&self) -> Vec<[usize; 3]> {
		let mut triangles = Vec::new();
		for face in self.faces() {
			self.triangulate(face, &mut triangles);
		}
		triangles
	}

	fn remove_vertex(&mut self, index: usize) {
		self.vertices.remove(index);
		self.edges.retain(|edge| !edge.contains(&index));

		for vertex in self.edges.iter_mut().flatten() {
			if *vertex > index {
				*vertex -= 1;
			}
		}
	}

	fn faces(&self) -> Vec<Vec<usize>> {
		let mut neighbours = vec![Vec::new(); self.vertices.len()];
		for &[a, b] in &self.edges {
			neighbours[a].push(b);
			neighbours[b].push(a);
		}

		let mut leaves: Vec<usize> = (0..neighbours.len())
			.filter(|&vertex| neighbours[vertex].len() == 1)
			.collect();
		while let Some(leaf) = leaves.pop() {
			let Some(neighbour) = neighbours[leaf].pop() else {
				continue;
			};

			neighbours[neighbour].retain(|&vertex| vertex != leaf);
			if neighbours[neighbour].len() == 1 {
				leaves.push(neighbour);
			}
		}

		for (vertex, list) in neighbours.iter_mut().enumerate() {
			let origin = self.vertices[vertex];
			list.sort_by(|&a, &b| {
				(self.vertices[a] - origin)
					.angle()
					.total_cmp(&(self.vertices[b] - origin).angle())
			});
		}

		let mut visited: Vec<Vec<bool>> = neighbours
			.iter()
			.map(|list| vec![false; list.len()])
			.collect();
		let mut faces = Vec::new();

		for start in 0..neighbours.len() {
			for start_slot in 0..neighbours[start].len() {
				let mut face = Vec::new();
				let (mut from, mut slot) = (start, start_slot);
				while !visited[from][slot] {
					visited[from][slot] = true;
					face.push(from);

					let to = neighbours[from][slot];
					let count = neighbours[to].len();
					let back = neighbours[to]
						.iter()
						.position(|&vertex| vertex == from)
						.unwrap();
					slot = (back + count - 1) % count;
					from = to;
				}

				if self.signed_area(&face) > 0.0 {
					faces.push(face);
				}
			}
		}

		faces
	}

	fn signed_area(&self, face: &[usize]) -> f32 {
		let mut area = 0.0;
		for (index, &vertex) in face.iter().enumerate() {
			let a = self.vertices[vertex];
			let b = self.vertices[face[(index + 1) % face.len()]];
			area += a.x * b.y - b.x * a.y;
		}
		area / 2.0
	}

	fn triangulate(&self, mut face: Vec<usize>, triangles: &mut Vec<[usize; 3]>) {
		while face.len() > 3 {
			let count = face.len();
			let corner = |index: usize| {
				[
					face[(index + count - 1) % count],
					face[index],
					face[(index + 1) % count],
				]
			};

			let ear = (0..count).find(|&index| {
				let [a, b, c] = corner(index).map(|vertex| self.vertices[vertex]);
				cross(b - a, c - b) > 0.0
					&& !face.iter().any(|&vertex| {
						let p = self.vertices[vertex];
						p != a && p != b && p != c && inside_triangle(p, a, b, c)
					})
			});

			let Some(ear) = ear else {
				return;
			};

			triangles.push(corner(ear));
			face.remove(ear);
		}

		if let [a, b, c] = face[..] {
			triangles.push([a, b, c]);
		}
	}
}

fn cross(a: Vec2, b: Vec2) -> f32 {
	a.x * b.y - a.y * b.x
}

fn inside_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
	cross(b - a, p - a) >= 0.0 && cross(c - b, p - b) >= 0.0 && cross(a - c, p - c) >= 0.0
}
