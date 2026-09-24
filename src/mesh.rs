use eframe::egui::{Pos2, Rect, Vec2};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct Mesh {
	pub vertices: Vec<Pos2>,
	pub edges: Vec<[usize; 2]>,
	pub layers: Vec<u32>,
	vertex_layers: Vec<u32>,
	holes: Vec<Vec<usize>>,
}

impl Mesh {
	pub fn add_vertex(&mut self, pos: Pos2) -> usize {
		let layer = self.new_layer();
		self.layers.insert(0, layer);
		self.vertex_layers.push(layer);
		self.vertices.push(pos);
		self.vertices.len() - 1
	}

	pub fn add_loop(&mut self, points: &[Pos2]) -> Vec<usize> {
		let layer = self.new_layer();
		self.layers.insert(0, layer);

		let offset = self.vertices.len();
		for (index, &pos) in points.iter().enumerate() {
			self.vertices.push(pos);
			self.vertex_layers.push(layer);
			self.edges
				.push([offset + index, offset + (index + 1) % points.len()]);
		}
		(offset..self.vertices.len()).collect()
	}

	pub fn extrude(&mut self, vertices: &[usize]) -> Vec<usize> {
		let copies = self.copy_vertices(vertices);
		for (&source, &copy) in vertices.iter().zip(&copies) {
			self.edges.push([source, copy]);
		}

		self.sync_layers();
		copies
	}

	pub fn duplicate(&mut self, vertices: &[usize]) -> Vec<usize> {
		let copies = self.copy_vertices(vertices);
		self.sync_layers();
		copies
	}

	pub fn layer(&self, vertex: usize) -> u32 {
		self.vertex_layers[vertex]
	}

	pub fn layer_vertices(&self, layer: u32) -> impl Iterator<Item = usize> {
		self.vertex_layers
			.iter()
			.enumerate()
			.filter(move |&(_, &id)| id == layer)
			.map(|(vertex, _)| vertex)
	}

	pub fn add_edge(&mut self, a: usize, b: usize) {
		if a == b || self.has_edge(a, b) {
			return;
		}

		self.edges.push([a, b]);
		self.sync_layers();
	}

	pub fn merge(&mut self, vertices: &[usize], pos: Pos2) -> usize {
		let target = *vertices.iter().min().unwrap();
		self.vertices[target] = pos;

		let map = |vertex: usize| {
			if vertices.contains(&vertex) {
				target
			} else {
				vertex
			}
		};
		for [a, b] in std::mem::take(&mut self.edges) {
			let (a, b) = (map(a), map(b));
			if a != b && !self.has_edge(a, b) {
				self.edges.push([a, b]);
			}
		}

		for hole in &mut self.holes {
			*hole = face_key(&hole.iter().map(|&vertex| map(vertex)).collect::<Vec<_>>());
		}

		let removed = vertices.iter().copied().filter(|&vertex| vertex != target);
		self.remove_vertices(removed.collect());
		target
	}

	pub fn dissolve(&mut self, vertices: Vec<usize>) {
		for &vertex in &vertices {
			let neighbours: Vec<usize> = self
				.edges
				.iter()
				.filter(|edge| edge.contains(&vertex))
				.map(|&[a, b]| if a == vertex { b } else { a })
				.collect();
			self.edges.retain(|edge| !edge.contains(&vertex));
			if let [a, b] = neighbours[..]
				&& !self.has_edge(a, b)
			{
				self.edges.push([a, b]);
			}
		}

		self.remove_vertices(vertices);
	}

	pub fn toggle_hole(&mut self, vertices: &[usize]) {
		let faces: Vec<Vec<usize>> = self
			.faces()
			.iter()
			.map(|face| face_key(face))
			.filter(|key| key.iter().all(|vertex| vertices.contains(vertex)))
			.collect();
		let fill = faces.iter().all(|key| self.holes.contains(key));
		self.holes.retain(|hole| !faces.contains(hole));
		if !fill {
			self.holes.extend(faces);
		}
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

	pub fn vertices_within(&self, pos: Pos2, radius: f32) -> impl Iterator<Item = usize> {
		self.vertices
			.iter()
			.enumerate()
			.filter(move |(_, vertex)| vertex.distance_sq(pos) <= radius * radius)
			.map(|(index, _)| index)
	}

	pub fn vertices_in(&self, rect: Rect) -> impl Iterator<Item = usize> {
		self.vertices
			.iter()
			.enumerate()
			.filter(move |(_, vertex)| rect.contains(**vertex))
			.map(|(index, _)| index)
	}

	pub fn face_at(&self, pos: Pos2) -> Option<Vec<usize>> {
		let ranks = self.ranks();
		let key = |face: &Vec<usize>| (ranks[&self.vertex_layers[face[0]]], self.signed_area(face));
		self.filled_faces()
			.into_iter()
			.filter(|face| self.encloses(face, pos))
			.min_by(|a, b| {
				let (a, b) = (key(a), key(b));
				a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))
			})
	}

	pub fn linked(&self, vertices: &[usize]) -> Vec<usize> {
		let (components, _) = self.components();
		let selected: HashSet<usize> = vertices.iter().map(|&vertex| components[vertex]).collect();
		(0..self.vertices.len())
			.filter(|&vertex| selected.contains(&components[vertex]))
			.collect()
	}

	pub fn remove_vertices(&mut self, mut indices: Vec<usize>) {
		indices.sort_unstable();
		for index in indices.into_iter().rev() {
			self.remove_vertex(index);
		}
		self.sync_layers();
	}

	pub fn triangles(&self) -> Vec<[usize; 3]> {
		let mut triangles = Vec::new();
		for face in self.filled_faces() {
			self.triangulate(face, &mut triangles);
		}

		let ranks = self.ranks();
		triangles.sort_by_key(|&[a, _, _]| std::cmp::Reverse(ranks[&self.vertex_layers[a]]));
		triangles
	}

	fn copy_vertices(&mut self, vertices: &[usize]) -> Vec<usize> {
		let offset = self.vertices.len();
		let copies: HashMap<usize, usize> = vertices
			.iter()
			.enumerate()
			.map(|(index, &vertex)| (vertex, offset + index))
			.collect();

		for &vertex in vertices {
			self.vertices.push(self.vertices[vertex]);
			self.vertex_layers.push(self.vertex_layers[vertex]);
		}

		for index in 0..self.edges.len() {
			let [a, b] = self.edges[index];
			if let (Some(&a), Some(&b)) = (copies.get(&a), copies.get(&b)) {
				self.edges.push([a, b]);
			}
		}

		for index in 0..self.holes.len() {
			let copy: Option<Vec<usize>> = self.holes[index]
				.iter()
				.map(|vertex| copies.get(vertex).copied())
				.collect();
			if let Some(copy) = copy {
				self.holes.push(face_key(&copy));
			}
		}

		(offset..self.vertices.len()).collect()
	}

	fn has_edge(&self, a: usize, b: usize) -> bool {
		self.edges
			.iter()
			.any(|&edge| edge == [a, b] || edge == [b, a])
	}

	fn ranks(&self) -> HashMap<u32, usize> {
		self.layers
			.iter()
			.enumerate()
			.map(|(rank, &layer)| (layer, rank))
			.collect()
	}

	fn new_layer(&self) -> u32 {
		(1..).find(|id| !self.layers.contains(id)).unwrap()
	}

	fn sync_layers(&mut self) {
		let (components, count) = self.components();
		let ranks = self.ranks();
		let mut sizes: HashMap<u32, usize> = HashMap::new();
		for &layer in &self.vertex_layers {
			*sizes.entry(layer).or_default() += 1;
		}

		let key = |layer: u32| (std::cmp::Reverse(sizes[&layer]), ranks[&layer]);
		let mut owners: Vec<Option<u32>> = vec![None; count];
		for (&component, &layer) in components.iter().zip(&self.vertex_layers) {
			let owner = &mut owners[component];
			if owner.is_none_or(|current| key(layer) < key(current)) {
				*owner = Some(layer);
			}
		}

		let mut owners: Vec<u32> = owners.into_iter().flatten().collect();
		let used: HashSet<u32> = owners.iter().copied().collect();
		self.layers.retain(|layer| used.contains(layer));

		let mut claimed = HashSet::new();
		for owner in &mut owners {
			if claimed.insert(*owner) {
				continue;
			}

			let layer = self.new_layer();
			let position = self.layers.iter().position(|&id| id == *owner).unwrap();
			self.layers.insert(position + 1, layer);
			*owner = layer;
		}

		for (layer, &component) in self.vertex_layers.iter_mut().zip(&components) {
			*layer = owners[component];
		}
	}

	fn components(&self) -> (Vec<usize>, usize) {
		let neighbours = self.neighbours();
		let mut components = vec![usize::MAX; self.vertices.len()];
		let mut count = 0;

		for start in 0..components.len() {
			if components[start] != usize::MAX {
				continue;
			}

			components[start] = count;
			let mut stack = vec![start];
			while let Some(vertex) = stack.pop() {
				for &next in &neighbours[vertex] {
					if components[next] == usize::MAX {
						components[next] = count;
						stack.push(next);
					}
				}
			}
			count += 1;
		}

		(components, count)
	}

	fn neighbours(&self) -> Vec<Vec<usize>> {
		let mut neighbours = vec![Vec::new(); self.vertices.len()];
		for &[a, b] in &self.edges {
			neighbours[a].push(b);
			neighbours[b].push(a);
		}
		neighbours
	}

	fn remove_vertex(&mut self, index: usize) {
		self.vertices.remove(index);
		self.vertex_layers.remove(index);
		self.edges.retain(|edge| !edge.contains(&index));

		for vertex in self.edges.iter_mut().flatten() {
			if *vertex > index {
				*vertex -= 1;
			}
		}

		for hole in &mut self.holes {
			hole.retain(|&vertex| vertex != index);
			for vertex in hole.iter_mut() {
				if *vertex > index {
					*vertex -= 1;
				}
			}
		}
		self.holes.retain(|hole| hole.len() >= 3);
	}

	fn filled_faces(&self) -> Vec<Vec<usize>> {
		self.faces()
			.into_iter()
			.filter(|face| !self.holes.contains(&face_key(face)))
			.collect()
	}

	fn faces(&self) -> Vec<Vec<usize>> {
		let mut neighbours = self.neighbours();

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

	fn encloses(&self, face: &[usize], pos: Pos2) -> bool {
		let mut inside = false;
		for (index, &vertex) in face.iter().enumerate() {
			let a = self.vertices[vertex];
			let b = self.vertices[face[(index + 1) % face.len()]];
			if (a.y > pos.y) != (b.y > pos.y)
				&& pos.x < a.x + (pos.y - a.y) * (b.x - a.x) / (b.y - a.y)
			{
				inside = !inside;
			}
		}
		inside
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

fn face_key(face: &[usize]) -> Vec<usize> {
	let mut key = face.to_vec();
	key.sort_unstable();
	key.dedup();
	key
}

fn cross(a: Vec2, b: Vec2) -> f32 {
	a.x * b.y - a.y * b.x
}

fn inside_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
	cross(b - a, p - a) >= 0.0 && cross(c - b, p - b) >= 0.0 && cross(a - c, p - c) >= 0.0
}
