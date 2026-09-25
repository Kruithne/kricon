use crate::history::Splice;
use crate::images::Image;
use eframe::egui::{Color32, Pos2, Rect, Vec2, vec2};
use std::collections::{HashMap, HashSet};

const MIN_MITER: f32 = 0.01;
const FACE_COLOR: Color32 = Color32::WHITE;

#[derive(Clone, Default)]
pub struct Mesh {
	pub vertices: Vec<Pos2>,
	pub edges: Vec<[usize; 2]>,
	pub layers: Vec<u32>,
	vertex_layers: Vec<u32>,
	holes: Vec<Vec<usize>>,
	colors: Vec<(Vec<usize>, Color32)>,
	pub images: Vec<Image>,
}

pub struct Change {
	vertices: Splice<Pos2>,
	edges: Splice<[usize; 2]>,
	layers: Splice<u32>,
	vertex_layers: Splice<u32>,
	holes: Splice<Vec<usize>>,
	colors: Splice<(Vec<usize>, Color32)>,
	images: Splice<Image>,
}

impl Change {
	pub fn is_empty(&self) -> bool {
		self.vertices.is_empty()
			&& self.edges.is_empty()
			&& self.layers.is_empty()
			&& self.vertex_layers.is_empty()
			&& self.holes.is_empty()
			&& self.colors.is_empty()
			&& self.images.is_empty()
	}
}

impl Mesh {
	pub fn diff(&self, after: &Mesh) -> Change {
		Change {
			vertices: Splice::new(&self.vertices, &after.vertices),
			edges: Splice::new(&self.edges, &after.edges),
			layers: Splice::new(&self.layers, &after.layers),
			vertex_layers: Splice::new(&self.vertex_layers, &after.vertex_layers),
			holes: Splice::new(&self.holes, &after.holes),
			colors: Splice::new(&self.colors, &after.colors),
			images: Splice::new(&self.images, &after.images),
		}
	}

	pub fn apply(&mut self, change: &Change, forward: bool) {
		change.vertices.apply(&mut self.vertices, forward);
		change.edges.apply(&mut self.edges, forward);
		change.layers.apply(&mut self.layers, forward);
		change.vertex_layers.apply(&mut self.vertex_layers, forward);
		change.holes.apply(&mut self.holes, forward);
		change.colors.apply(&mut self.colors, forward);
		change.images.apply(&mut self.images, forward);
	}

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

	pub fn inset(&mut self, vertices: &[usize]) -> (Vec<usize>, Vec<Vec2>) {
		let faces: Vec<Vec<usize>> = self
			.faces()
			.into_iter()
			.filter(|face| face.iter().all(|vertex| vertices.contains(vertex)))
			.collect();

		let mut counts: HashMap<Vec<usize>, usize> = HashMap::new();
		let mut directed = Vec::new();
		for face in &faces {
			for (index, &a) in face.iter().enumerate() {
				let b = face[(index + 1) % face.len()];
				*counts.entry(face_key(&[a, b])).or_default() += 1;
				directed.push([a, b]);
			}
		}

		let boundary: Vec<[usize; 2]> = directed
			.into_iter()
			.filter(|&[a, b]| counts[&face_key(&[a, b])] == 1)
			.collect();
		let next: HashMap<usize, usize> = boundary.iter().map(|&[a, b]| (a, b)).collect();
		let prev: HashMap<usize, usize> = boundary.iter().map(|&[a, b]| (b, a)).collect();

		let mut copies = HashMap::new();
		let mut directions = Vec::new();
		for &[vertex, _] in &boundary {
			if copies.contains_key(&vertex) {
				continue;
			}

			let pos = self.vertices[vertex];
			let inward = |from: Pos2, to: Pos2| {
				let edge = to - from;
				vec2(-edge.y, edge.x).normalized()
			};
			let a = inward(self.vertices[prev[&vertex]], pos);
			let b = inward(pos, self.vertices[next[&vertex]]);
			directions.push((a + b) / (1.0 + a.dot(b)).max(MIN_MITER));

			copies.insert(vertex, self.vertices.len());
			self.vertices.push(pos);
			self.vertex_layers.push(self.vertex_layers[vertex]);
		}

		let map = |vertex: usize| copies.get(&vertex).copied().unwrap_or(vertex);
		for edge in &mut self.edges {
			if counts.get(&face_key(edge)) == Some(&2) {
				*edge = edge.map(map);
			}
		}

		let keys: Vec<Vec<usize>> = faces.iter().map(|face| face_key(face)).collect();
		for key in self.face_keys() {
			if keys.contains(key) {
				*key = face_key(&key.iter().map(|&vertex| map(vertex)).collect::<Vec<_>>());
			}
		}

		for &[a, b] in &boundary {
			self.edges.push([map(a), map(b)]);
		}

		let mut inner = Vec::new();
		for &[vertex, _] in &boundary {
			let copy = map(vertex);
			if !inner.contains(&copy) {
				self.edges.push([vertex, copy]);
				inner.push(copy);
			}
		}
		(inner, directions)
	}

	pub fn duplicate(&mut self, vertices: &[usize]) -> Vec<usize> {
		let copies = self.copy_vertices(vertices);
		self.sync_layers();
		copies
	}

	pub fn move_layer(&mut self, from: usize, target: usize) {
		let layer = self.layers.remove(from);
		let to = if target > from { target - 1 } else { target };
		self.layers.insert(to, layer);
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

		for key in self.face_keys() {
			*key = face_key(&key.iter().map(|&vertex| map(vertex)).collect::<Vec<_>>());
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

	pub fn subdivide(&mut self, vertices: &[usize], smooth: bool) -> Vec<usize> {
		let neighbours = self.neighbours();
		let targets: Vec<(usize, Pos2)> = self
			.edges
			.iter()
			.enumerate()
			.filter(|(_, [a, b])| vertices.contains(a) && vertices.contains(b))
			.map(|(index, &[a, b])| {
				let pos = if smooth {
					self.arc_midpoint(&neighbours, a, b)
				} else {
					self.vertices[a].lerp(self.vertices[b], 0.5)
				};
				(index, pos)
			})
			.collect();

		let mut midpoints = Vec::new();
		for (index, pos) in targets {
			let [a, b] = self.edges[index];
			let midpoint = self.vertices.len();
			self.vertices.push(pos);
			self.vertex_layers.push(self.vertex_layers[a]);
			self.edges[index] = [a, midpoint];
			self.edges.push([midpoint, b]);

			for key in self.face_keys() {
				if key.contains(&a) && key.contains(&b) {
					key.push(midpoint);
				}
			}
			midpoints.push(midpoint);
		}
		midpoints
	}

	pub fn decimate(&mut self, vertices: &[usize]) -> Vec<usize> {
		let neighbours = self.neighbours();
		let interior = |vertex: usize| {
			vertices.contains(&vertex)
				&& neighbours[vertex].len() == 2
				&& neighbours[vertex]
					.iter()
					.all(|next| vertices.contains(next))
		};

		let mut visited = HashSet::new();
		let mut walk = |mut prev: usize, mut current: usize| {
			let mut chain = Vec::new();
			while interior(current) && visited.insert(current) {
				chain.push(current);
				let next = neighbours[current].iter().find(|&&next| next != prev);
				(prev, current) = (current, *next.unwrap());
			}
			chain
		};

		let mut removed = Vec::new();
		for &anchor in vertices.iter().filter(|&&vertex| !interior(vertex)) {
			for &next in &neighbours[anchor] {
				removed.extend(walk(anchor, next).into_iter().step_by(2));
			}
		}

		for &start in vertices {
			if interior(start) {
				let chain = walk(neighbours[start][0], start);
				if chain.len() > 4 {
					removed.extend(chain.into_iter().skip(1).step_by(2));
				}
			}
		}

		let kept = vertices
			.iter()
			.filter(|vertex| !removed.contains(vertex))
			.map(|&vertex| vertex - removed.iter().filter(|&&index| index < vertex).count())
			.collect();
		self.dissolve(removed);
		kept
	}

	pub fn toggle_hole(&mut self, vertices: &[usize]) {
		let faces = self.enclosed_faces(vertices);
		let fill = faces.iter().all(|key| self.holes.contains(key));
		self.holes.retain(|hole| !faces.contains(hole));
		if !fill {
			self.holes.extend(faces);
		}
	}

	pub fn face_color(&self, vertices: &[usize]) -> Option<Color32> {
		let face = self.enclosed_faces(vertices).into_iter().next()?;
		Some(self.color_of(&face))
	}

	pub fn set_color(&mut self, vertices: &[usize], color: Color32) {
		let faces = self.enclosed_faces(vertices);
		self.colors.retain(|(key, _)| !faces.contains(key));
		self.colors
			.extend(faces.into_iter().map(|face| (face, color)));
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

	pub fn image_at(&self, pos: Pos2) -> Option<usize> {
		self.images.iter().rposition(|image| image.contains(pos))
	}

	pub fn remove_images(&mut self, mut indices: Vec<usize>) {
		indices.sort_unstable();
		for index in indices.into_iter().rev() {
			self.images.remove(index);
		}
	}

	pub fn remove_vertices(&mut self, mut indices: Vec<usize>) {
		indices.sort_unstable();
		for index in indices.into_iter().rev() {
			self.remove_vertex(index);
		}
		self.sync_layers();
	}

	pub fn triangles(&self) -> Vec<([usize; 3], Color32)> {
		let mut triangles = Vec::new();
		for face in self.filled_faces() {
			let color = self.color_of(&face_key(&face));
			for triangle in self.triangulate(face) {
				triangles.push((triangle, color));
			}
		}

		let ranks = self.ranks();
		triangles.sort_by_key(|&([a, _, _], _)| std::cmp::Reverse(ranks[&self.vertex_layers[a]]));
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

		for index in 0..self.colors.len() {
			let (key, color) = &self.colors[index];
			let copy: Option<Vec<usize>> = key
				.iter()
				.map(|vertex| copies.get(vertex).copied())
				.collect();
			if let Some(copy) = copy {
				self.colors.push((face_key(&copy), *color));
			}
		}

		(offset..self.vertices.len()).collect()
	}

	fn arc_midpoint(&self, neighbours: &[Vec<usize>], a: usize, b: usize) -> Pos2 {
		let other = |vertex: usize, skip: usize| match neighbours[vertex][..] {
			[x, y] => Some(if x == skip { y } else { x }),
			_ => None,
		};

		let (pa, pb) = (self.vertices[a], self.vertices[b]);
		let from_a = other(a, b).and_then(|p| arc_point(self.vertices[p], pa, pb));
		let from_b = other(b, a).and_then(|q| arc_point(self.vertices[q], pa, pb));
		match (from_a, from_b) {
			(Some(x), Some(y)) => x.lerp(y, 0.5),
			(Some(point), None) | (None, Some(point)) => point,
			(None, None) => pa.lerp(pb, 0.5),
		}
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

		for key in self.face_keys() {
			key.retain(|&vertex| vertex != index);
			for vertex in key.iter_mut() {
				if *vertex > index {
					*vertex -= 1;
				}
			}
		}
		self.holes.retain(|hole| hole.len() >= 3);
		self.colors.retain(|(key, _)| key.len() >= 3);
	}

	fn face_keys(&mut self) -> impl Iterator<Item = &mut Vec<usize>> {
		self.holes
			.iter_mut()
			.chain(self.colors.iter_mut().map(|(key, _)| key))
	}

	fn enclosed_faces(&self, vertices: &[usize]) -> Vec<Vec<usize>> {
		self.faces()
			.iter()
			.map(|face| face_key(face))
			.filter(|key| key.iter().all(|vertex| vertices.contains(vertex)))
			.collect()
	}

	fn color_of(&self, key: &[usize]) -> Color32 {
		self.colors
			.iter()
			.find(|(face, _)| face == key)
			.map_or(FACE_COLOR, |&(_, color)| color)
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
		let points: Vec<Pos2> = face.iter().map(|&vertex| self.vertices[vertex]).collect();
		encloses(&points, pos)
	}

	fn triangulate(&self, mut face: Vec<usize>) -> Vec<[usize; 3]> {
		let mut triangles = Vec::new();
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
				return triangles;
			};

			triangles.push(corner(ear));
			face.remove(ear);
		}

		if let [a, b, c] = face[..] {
			triangles.push([a, b, c]);
		}
		triangles
	}
}

pub fn encloses(polygon: &[Pos2], pos: Pos2) -> bool {
	let mut inside = false;
	for (index, &a) in polygon.iter().enumerate() {
		let b = polygon[(index + 1) % polygon.len()];
		if (a.y > pos.y) != (b.y > pos.y) && pos.x < a.x + (pos.y - a.y) * (b.x - a.x) / (b.y - a.y)
		{
			inside = !inside;
		}
	}
	inside
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

fn arc_point(p: Pos2, a: Pos2, b: Pos2) -> Option<Pos2> {
	let (u, v) = (a - p, b - p);
	let denominator = u.length() * v.length() + u.dot(v);
	if denominator <= f32::EPSILON {
		return None;
	}

	let chord = b - a;
	let tangent = cross(chord, p - a) / denominator;
	Some(a.lerp(b, 0.5) + Vec2::new(chord.y, -chord.x) * tangent / 2.0)
}

fn inside_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
	cross(b - a, p - a) >= 0.0 && cross(c - b, p - b) >= 0.0 && cross(a - c, p - c) >= 0.0
}
