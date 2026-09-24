use eframe::egui::Pos2;

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

	fn remove_vertex(&mut self, index: usize) {
		self.vertices.remove(index);
		self.edges.retain(|edge| !edge.contains(&index));

		for vertex in self.edges.iter_mut().flatten() {
			if *vertex > index {
				*vertex -= 1;
			}
		}
	}
}
