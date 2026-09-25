use crate::export::Fill;
use crate::geometry::{Segment, area, bspline_span, cross, encloses, turn};
use crate::history::Splice;
use crate::images::Image;
use crate::import::Shape;
use eframe::egui::{Color32, Pos2, Rect, Vec2, vec2};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

const MIN_MITER: f32 = 0.01;
const FACE_COLOR: Color32 = Color32::WHITE;
const CURVE_SEGMENTS: usize = 16;
const SKEW_TOLERANCE: f32 = 0.1;
const MIN_PIECE_AREA: f32 = 1e-6;

#[derive(Clone, PartialEq)]
pub struct Layer {
	pub id: u32,
	pub curve: bool,
	pub holdout: bool,
	pub name: String,
}

#[derive(Clone, Default)]
pub struct Mesh {
	pub vertices: Vec<Pos2>,
	pub edges: Vec<[usize; 2]>,
	pub layers: Vec<Layer>,
	pub vertex_layers: Vec<u32>,
	pub holes: Vec<Vec<usize>>,
	pub colors: Vec<(Vec<usize>, Color32)>,
	pub images: Vec<Image>,
}

pub struct Change {
	vertices: Splice<Pos2>,
	edges: Splice<[usize; 2]>,
	layers: Splice<Layer>,
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
		self.layers.insert(
			0,
			Layer {
				id: layer,
				curve: false,
				holdout: false,
				name: String::new(),
			},
		);
		self.vertex_layers.push(layer);
		self.vertices.push(pos);
		self.vertices.len() - 1
	}

	pub fn add_loop(&mut self, points: &[Pos2], curve: bool) -> Vec<usize> {
		let layer = self.new_layer();
		self.layers.insert(
			0,
			Layer {
				id: layer,
				curve,
				holdout: false,
				name: String::new(),
			},
		);

		let offset = self.vertices.len();
		for (index, &pos) in points.iter().enumerate() {
			self.vertices.push(pos);
			self.vertex_layers.push(layer);
			self.edges
				.push([offset + index, offset + (index + 1) % points.len()]);
		}
		(offset..self.vertices.len()).collect()
	}

	pub fn add_shape(&mut self, shape: &Shape, offset: Vec2) -> Vec<usize> {
		let points: Vec<Pos2> = shape.points.iter().map(|&pos| pos + offset).collect();
		let vertices = self.add_loop(&points, shape.curve);
		if shape.holdout {
			self.toggle_holdout(&vertices);
		}
		if let Some(color) = shape.color {
			self.set_color(&vertices, color);
		}
		vertices
	}

	pub fn extrude(&mut self, vertices: &[usize]) -> Vec<usize> {
		let copies = self.copy_vertices(vertices);
		for (&source, &copy) in vertices.iter().zip(&copies) {
			self.edges.push([source, copy]);
		}

		self.sync_layers(false);
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
		self.sync_layers(true);
		copies
	}

	pub fn move_layer(&mut self, from: usize, target: usize) {
		let layer = self.layers.remove(from);
		let to = if target > from { target - 1 } else { target };
		self.layers.insert(to, layer);
	}

	pub fn rename_layer(&mut self, id: u32, name: String) {
		if let Some(layer) = self.layers.iter_mut().find(|layer| layer.id == id) {
			layer.name = name;
		}
	}

	pub fn layer(&self, vertex: usize) -> u32 {
		self.vertex_layers[vertex]
	}

	pub fn is_curve(&self, vertex: usize) -> bool {
		let id = self.vertex_layers[vertex];
		self.layers
			.iter()
			.any(|layer| layer.id == id && layer.curve)
	}

	pub fn toggle_holdout(&mut self, vertices: &[usize]) {
		let ids: HashSet<u32> = vertices
			.iter()
			.map(|&vertex| self.vertex_layers[vertex])
			.collect();
		let mut targets: Vec<&mut Layer> = self
			.layers
			.iter_mut()
			.filter(|layer| ids.contains(&layer.id))
			.collect();
		let holdout = !targets.iter().all(|layer| layer.holdout);
		for layer in &mut targets {
			layer.holdout = holdout;
		}
	}

	pub fn curve_outlines(&self) -> Vec<Vec<Pos2>> {
		self.faces()
			.iter()
			.filter(|face| self.is_curve(face[0]))
			.map(|face| self.outline(face))
			.collect()
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
		self.sync_layers(false);
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

	pub fn space(&mut self, vertices: &[usize]) {
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
			let mut chain = vec![prev];
			while interior(current) && visited.insert(current) {
				chain.push(current);
				let next = neighbours[current].iter().find(|&&next| next != prev);
				(prev, current) = (current, *next.unwrap());
			}
			chain.push(current);
			chain
		};

		let mut chains = Vec::new();
		for &anchor in vertices.iter().filter(|&&vertex| !interior(vertex)) {
			for &next in &neighbours[anchor] {
				chains.push(walk(anchor, next));
			}
		}

		for &start in vertices {
			if interior(start) {
				chains.push(walk(neighbours[start][0], start).split_off(1));
			}
		}

		for chain in chains.iter().filter(|chain| chain.len() > 2) {
			self.distribute(chain);
		}
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
			.filter(|face| encloses(&self.outline(face), pos))
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

	pub fn edge_loops(&self, vertices: &[usize]) -> Vec<usize> {
		let neighbours = self.neighbours();
		let mut found = Vec::new();
		for &[a, b] in &self.edges {
			if !vertices.contains(&a) || !vertices.contains(&b) {
				continue;
			}

			for (mut prev, mut current) in [(a, b), (b, a)] {
				let mut visited = HashSet::from([prev]);
				while visited.insert(current) {
					found.push(current);
					let Some(next) = self.loop_next(&neighbours, prev, current) else {
						break;
					};
					(prev, current) = (current, next);
				}
			}
		}
		found
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
		self.sync_layers(false);
	}

	pub fn triangles(&self) -> Vec<([Pos2; 3], Color32)> {
		let ranks = self.ranks();
		let holdouts = self.holdouts();
		let rank = |face: &[usize]| ranks[&self.vertex_layers[face[0]]];
		let (cutters, mut faces): (Vec<_>, Vec<_>) = self
			.filled_faces()
			.into_iter()
			.partition(|face| holdouts.contains(&self.vertex_layers[face[0]]));
		faces.sort_by_key(|face| Reverse(rank(face)));

		let cutters: Vec<(usize, [Pos2; 3])> = cutters
			.iter()
			.flat_map(|face| {
				triangulate(self.outline(face))
					.into_iter()
					.map(move |triangle| (rank(face), triangle))
			})
			.collect();

		let mut triangles = Vec::new();
		for face in faces {
			let color = self.color_of(&face_key(&face));
			let above: Vec<&[Pos2; 3]> = cutters
				.iter()
				.filter(|(cutter, _)| *cutter < rank(&face))
				.map(|(_, triangle)| triangle)
				.collect();

			for triangle in triangulate(self.outline(&face)) {
				for piece in subtract(triangle.to_vec(), &above) {
					for index in 1..piece.len() - 1 {
						triangles.push(([piece[0], piece[index], piece[index + 1]], color));
					}
				}
			}
		}
		triangles
	}

	pub fn regions(&self) -> Vec<(Vec<Pos2>, Option<Color32>)> {
		let ranks = self.ranks();
		let holdouts = self.holdouts();
		let mut faces = self.filled_faces();
		faces.sort_by_key(|face| ranks[&self.vertex_layers[face[0]]]);
		faces
			.iter()
			.map(|face| {
				let holdout = holdouts.contains(&self.vertex_layers[face[0]]);
				let fill = (!holdout).then(|| self.color_of(&face_key(face)));
				(self.outline(face), fill)
			})
			.collect()
	}

	pub fn fills(&self, vertices: &[usize]) -> Vec<Fill> {
		let selected: HashSet<usize> = vertices.iter().copied().collect();
		let ranks = self.ranks();
		let holdouts = self.holdouts();
		let rank = |face: &[usize]| ranks[&self.vertex_layers[face[0]]];
		let (cutters, faces): (Vec<_>, Vec<_>) = self
			.filled_faces()
			.into_iter()
			.partition(|face| holdouts.contains(&self.vertex_layers[face[0]]));

		let mut fills: Vec<(usize, Fill)> = Vec::new();
		for face in faces
			.iter()
			.filter(|face| face.iter().all(|vertex| selected.contains(vertex)))
		{
			let (rank, color) = (rank(face), self.color_of(&face_key(face)));
			let contour = self.segments(face);
			if let Some((_, fill)) = fills
				.iter_mut()
				.find(|(other, fill)| *other == rank && fill.color == color)
			{
				fill.contours.push(contour);
				continue;
			}

			let cutters = cutters
				.iter()
				.filter(|cutter| ranks[&self.vertex_layers[cutter[0]]] < rank)
				.map(|cutter| self.segments(cutter))
				.collect();
			fills.push((
				rank,
				Fill {
					color,
					contours: vec![contour],
					cutters,
				},
			));
		}

		fills.sort_by_key(|&(rank, _)| Reverse(rank));
		fills.into_iter().map(|(_, fill)| fill).collect()
	}

	fn segments(&self, face: &[usize]) -> Vec<Segment> {
		let count = face.len();
		let point = |index: usize| self.vertices[face[index % count]];
		let curve = self.is_curve(face[0]);
		(0..count)
			.map(|index| {
				if !curve {
					return Segment::Line([point(index), point(index + 1)]);
				}

				Segment::Cubic(bspline_span(
					[0, 1, 2, 3].map(|offset| point(index + offset)),
				))
			})
			.collect()
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

	fn loop_next(&self, neighbours: &[Vec<usize>], prev: usize, vertex: usize) -> Option<usize> {
		let origin = self.vertices[vertex];
		let direction = |to: usize| self.vertices[to] - origin;
		let angle = |from: usize, to: usize| {
			let (u, v) = (direction(from).normalized(), direction(to).normalized());
			u.dot(v).clamp(-1.0, 1.0).acos()
		};

		let others: Vec<usize> = neighbours[vertex]
			.iter()
			.copied()
			.filter(|&next| next != prev)
			.collect();
		match others[..] {
			[next] => Some(next),
			[a, b] => {
				let skew =
					|next: usize, branch: usize| (angle(branch, prev) - angle(branch, next)).abs();
				let (skew_a, skew_b) = (skew(a, b), skew(b, a));
				if (skew_a - skew_b).abs() < SKEW_TOLERANCE {
					None
				} else if skew_a < skew_b {
					Some(a)
				} else {
					Some(b)
				}
			}
			[_, _, _] => {
				let back = direction(prev);
				let mut sorted = others;
				sorted.sort_by(|&a, &b| {
					turn(back, direction(a)).total_cmp(&turn(back, direction(b)))
				});
				Some(sorted[1])
			}
			_ => None,
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
			.map(|(rank, layer)| (layer.id, rank))
			.collect()
	}

	fn holdouts(&self) -> HashSet<u32> {
		self.layers
			.iter()
			.filter(|layer| layer.holdout)
			.map(|layer| layer.id)
			.collect()
	}

	fn new_layer(&self) -> u32 {
		(1..)
			.find(|&id| !self.layers.iter().any(|layer| layer.id == id))
			.unwrap()
	}

	fn outline(&self, face: &[usize]) -> Vec<Pos2> {
		let points: Vec<Pos2> = face.iter().map(|&vertex| self.vertices[vertex]).collect();
		if self.is_curve(face[0]) {
			spline(&points)
		} else {
			points
		}
	}

	fn sync_layers(&mut self, above: bool) {
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
		self.layers.retain(|layer| used.contains(&layer.id));

		let mut claimed = HashSet::new();
		for owner in &mut owners {
			if claimed.insert(*owner) {
				continue;
			}

			let id = self.new_layer();
			let position = self
				.layers
				.iter()
				.position(|layer| layer.id == *owner)
				.unwrap();
			let Layer { curve, holdout, .. } = self.layers[position];
			self.layers.insert(
				position + usize::from(!above),
				Layer {
					id,
					curve,
					holdout,
					name: String::new(),
				},
			);
			*owner = id;
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

	fn distribute(&mut self, chain: &[usize]) {
		let points: Vec<Pos2> = chain.iter().map(|&vertex| self.vertices[vertex]).collect();
		let lengths: Vec<f32> = points
			.windows(2)
			.map(|pair| pair[0].distance(pair[1]))
			.collect();
		let step = lengths.iter().sum::<f32>() / lengths.len() as f32;

		let mut segment = 0;
		let mut start = 0.0;
		for (index, &vertex) in chain.iter().enumerate().take(chain.len() - 1).skip(1) {
			let target = step * index as f32;
			while segment < lengths.len() - 1 && start + lengths[segment] < target {
				start += lengths[segment];
				segment += 1;
			}

			let t = if lengths[segment] > 0.0 {
				(target - start) / lengths[segment]
			} else {
				0.0
			};
			self.vertices[vertex] = points[segment].lerp(points[segment + 1], t);
		}
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
		let points: Vec<Pos2> = face.iter().map(|&vertex| self.vertices[vertex]).collect();
		area(&points)
	}
}

fn subtract(polygon: Vec<Pos2>, cutters: &[&[Pos2; 3]]) -> Vec<Vec<Pos2>> {
	let mut pieces = vec![polygon];
	for cutter in cutters {
		let bounds = Rect::from_points(&cutter[..]);
		let mut kept = Vec::new();
		for piece in pieces {
			if !bounds.intersects(Rect::from_points(&piece)) {
				kept.push(piece);
				continue;
			}

			let mut inside = piece;
			for index in 0..3 {
				let [within, outside] = split(&inside, cutter[index], cutter[(index + 1) % 3]);
				if area(&outside) > MIN_PIECE_AREA {
					kept.push(outside);
				}
				if area(&within) <= MIN_PIECE_AREA {
					break;
				}
				inside = within;
			}
		}
		pieces = kept;
	}
	pieces
}

fn split(polygon: &[Pos2], a: Pos2, b: Pos2) -> [Vec<Pos2>; 2] {
	let side = |p: Pos2| cross(b - a, p - a);
	let mut halves = [Vec::new(), Vec::new()];
	for (index, &p) in polygon.iter().enumerate() {
		let q = polygon[(index + 1) % polygon.len()];
		let (sp, sq) = (side(p), side(q));
		if sp >= 0.0 {
			halves[0].push(p);
		}
		if sp <= 0.0 {
			halves[1].push(p);
		}
		if sp * sq < 0.0 {
			let point = p.lerp(q, sp / (sp - sq));
			halves[0].push(point);
			halves[1].push(point);
		}
	}
	halves
}

fn face_key(face: &[usize]) -> Vec<usize> {
	let mut key = face.to_vec();
	key.sort_unstable();
	key.dedup();
	key
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

fn spline(points: &[Pos2]) -> Vec<Pos2> {
	let count = points.len();
	let mut outline = Vec::with_capacity(count * CURVE_SEGMENTS);
	for index in 0..count {
		let [a, b, c, d] = [0, 1, 2, 3].map(|offset| points[(index + offset) % count].to_vec2());
		for step in 0..CURVE_SEGMENTS {
			let t = step as f32 / CURVE_SEGMENTS as f32;
			let u = 1.0 - t;
			let pos = a * (u * u * u)
				+ b * (3.0 * t * t * t - 6.0 * t * t + 4.0)
				+ c * (-3.0 * t * t * t + 3.0 * t * t + 3.0 * t + 1.0)
				+ d * (t * t * t);
			outline.push((pos / 6.0).to_pos2());
		}
	}
	outline
}

fn triangulate(mut polygon: Vec<Pos2>) -> Vec<[Pos2; 3]> {
	let mut triangles = Vec::new();
	while polygon.len() > 3 {
		let count = polygon.len();
		let corner = |index: usize| {
			[
				polygon[(index + count - 1) % count],
				polygon[index],
				polygon[(index + 1) % count],
			]
		};

		let ear = (0..count).find(|&index| {
			let [a, b, c] = corner(index);
			cross(b - a, c - b) > 0.0
				&& !polygon
					.iter()
					.any(|&p| p != a && p != b && p != c && inside_triangle(p, a, b, c))
		});

		let Some(ear) = ear else {
			return triangles;
		};

		triangles.push(corner(ear));
		polygon.remove(ear);
	}

	if let [a, b, c] = polygon[..] {
		triangles.push([a, b, c]);
	}
	triangles
}
