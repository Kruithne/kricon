use crate::geometry::cross;
use eframe::egui::{Color32, Pos2, Rect, Vec2, vec2};
use std::fmt::Write;

const FLATNESS: f32 = 1e-4;
const MAX_DEPTH: u32 = 24;
const SNAP: f32 = 1e-3;
const SAMPLE_OFFSET: f32 = 1e-4;
const PARALLEL: f32 = 1e-6;
const BISECT_STEPS: usize = 32;
const DECIMALS: usize = 3;

#[derive(Clone, Copy)]
pub enum Segment {
	Line([Pos2; 2]),
	Cubic([Pos2; 4]),
}

pub struct Fill {
	pub color: Color32,
	pub contours: Vec<Vec<Segment>>,
	pub cutters: Vec<Vec<Segment>>,
}

impl Segment {
	fn points(&self) -> &[Pos2] {
		match self {
			Self::Line(points) => points,
			Self::Cubic(points) => points,
		}
	}

	fn points_mut(&mut self) -> &mut [Pos2] {
		match self {
			Self::Line(points) => points,
			Self::Cubic(points) => points,
		}
	}

	fn start(&self) -> Pos2 {
		self.points()[0]
	}

	fn end(&self) -> Pos2 {
		self.points()[self.points().len() - 1]
	}

	fn set_start(&mut self, pos: Pos2) {
		self.points_mut()[0] = pos;
	}

	fn set_end(&mut self, pos: Pos2) {
		let points = self.points_mut();
		points[points.len() - 1] = pos;
	}

	fn translate(mut self, offset: Vec2) -> Self {
		for point in self.points_mut() {
			*point += offset;
		}
		self
	}

	fn reversed(self) -> Self {
		match self {
			Self::Line([a, b]) => Self::Line([b, a]),
			Self::Cubic([a, b, c, d]) => Self::Cubic([d, c, b, a]),
		}
	}

	fn split(&self, t: f32) -> (Self, Self) {
		match *self {
			Self::Line([a, b]) => {
				let middle = a.lerp(b, t);
				(Self::Line([a, middle]), Self::Line([middle, b]))
			}
			Self::Cubic([a, b, c, d]) => {
				let (ab, bc, cd) = (a.lerp(b, t), b.lerp(c, t), c.lerp(d, t));
				let (abc, bcd) = (ab.lerp(bc, t), bc.lerp(cd, t));
				let middle = abc.lerp(bcd, t);
				(
					Self::Cubic([a, ab, abc, middle]),
					Self::Cubic([middle, bcd, cd, d]),
				)
			}
		}
	}

	fn point(&self, t: f32) -> Pos2 {
		self.split(t).0.end()
	}

	fn hull(&self) -> Rect {
		Rect::from_points(self.points())
	}

	fn bounds(&self) -> Rect {
		let mut params = vec![0.0, 1.0];
		params.extend(self.extrema(0));
		params.extend(self.extrema(1));
		let points: Vec<Pos2> = params.into_iter().map(|t| self.point(t)).collect();
		Rect::from_points(&points)
	}

	fn is_flat(&self) -> bool {
		let Self::Cubic([a, b, c, d]) = *self else {
			return true;
		};

		let chord = d - a;
		let length = chord.length();
		let distance = |p: Pos2| {
			if length > f32::EPSILON {
				cross(chord, p - a).abs() / length
			} else {
				p.distance(a)
			}
		};
		distance(b).max(distance(c)) <= FLATNESS
	}

	fn extrema(&self, axis: usize) -> Vec<f32> {
		let Self::Cubic(points) = self else {
			return Vec::new();
		};

		let [a, b, c, d] = points.map(|point| point[axis]);
		let (qa, qb, qc) = (d - a + 3.0 * (b - c), 2.0 * (a - 2.0 * b + c), b - a);
		let mut roots = if qa.abs() > f32::EPSILON {
			let discriminant = qb * qb - 4.0 * qa * qc;
			if discriminant < 0.0 {
				Vec::new()
			} else {
				let root = discriminant.sqrt();
				vec![(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]
			}
		} else if qb.abs() > f32::EPSILON {
			vec![-qc / qb]
		} else {
			Vec::new()
		};
		roots.retain(|&t| 0.0 < t && t < 1.0);
		roots.sort_by(f32::total_cmp);
		roots
	}

	fn winding(&self, pos: Pos2) -> i32 {
		let hull = self.hull();
		if pos.y < hull.min.y || pos.y >= hull.max.y || pos.x >= hull.max.x {
			return 0;
		}

		let mut params = vec![0.0];
		params.extend(self.extrema(1));
		params.push(1.0);

		params
			.windows(2)
			.map(|range| {
				let (a, b) = (self.point(range[0]), self.point(range[1]));
				let up = a.y <= pos.y && pos.y < b.y;
				let down = b.y <= pos.y && pos.y < a.y;
				if !up && !down {
					return 0;
				}

				let x = match self {
					Self::Line(_) => a.x + (pos.y - a.y) * (b.x - a.x) / (b.y - a.y),
					Self::Cubic(_) => {
						let (mut low, mut high) = (range[0], range[1]);
						for _ in 0..BISECT_STEPS {
							let middle = (low + high) / 2.0;
							if (self.point(middle).y < pos.y) == up {
								low = middle;
							} else {
								high = middle;
							}
						}
						self.point((low + high) / 2.0).x
					}
				};

				match (x > pos.x, up) {
					(false, _) => 0,
					(true, true) => 1,
					(true, false) => -1,
				}
			})
			.sum()
	}

	fn sample(&self) -> (Pos2, Vec2) {
		let (left, right) = self.split(0.5);
		let tangent = match (left, right) {
			(Self::Cubic([.., a, _]), Self::Cubic([_, b, ..])) if a != b => b - a,
			_ => self.end() - self.start(),
		};
		(left.end(), vec2(-tangent.y, tangent.x).normalized())
	}

	fn pieces(&self, mut splits: Vec<(f32, Pos2)>) -> Vec<Self> {
		splits.sort_by(|a, b| a.0.total_cmp(&b.0));

		let mut pieces = Vec::new();
		let (mut rest, mut from, mut last) = (*self, 0.0, self.start());
		for (t, pos) in splits {
			if pos.distance(last) <= SNAP || pos.distance(self.end()) <= SNAP {
				continue;
			}

			let local = ((t - from) / (1.0 - from)).clamp(0.0, 1.0);
			let (mut piece, mut next) = rest.split(local);
			piece.set_end(pos);
			next.set_start(pos);
			pieces.push(piece);
			(rest, from, last) = (next, t, pos);
		}

		pieces.push(rest);
		pieces
	}

	fn matches(&self, other: &Self) -> bool {
		self.start().distance(other.start()) <= SNAP
			&& self.end().distance(other.end()) <= SNAP
			&& self.point(0.5).distance(other.point(0.5)) <= SNAP
	}
}

pub fn svg(fills: Vec<Fill>) -> Option<String> {
	let origin = fills
		.iter()
		.flat_map(|fill| fill.contours.iter().flatten())
		.map(Segment::bounds)
		.reduce(Rect::union)?
		.min
		.floor()
		.to_vec2();

	let paths: Vec<(Color32, Vec<Vec<Segment>>)> = fills
		.into_iter()
		.map(|fill| {
			let contours = translate(&fill.contours, -origin);
			let area = hull(&contours);
			let cutters: Vec<Vec<Segment>> = translate(&fill.cutters, -origin)
				.into_iter()
				.filter(|cutter| hull(std::slice::from_ref(cutter)).intersects(area))
				.collect();
			(fill.color, subtract(&contours, &cutters))
		})
		.filter(|(_, contours)| !contours.is_empty())
		.collect();

	let bounds = paths
		.iter()
		.flat_map(|(_, contours)| contours.iter().flatten())
		.map(Segment::bounds)
		.reduce(Rect::union)?;
	let min = (bounds.min + Vec2::splat(SNAP)).floor();
	let size = (bounds.max - Vec2::splat(SNAP)).ceil() - min;

	let (width, height) = (number(size.x), number(size.y));
	let mut svg = format!(
		"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n"
	);
	for (color, contours) in paths {
		let [r, g, b, _] = color.to_array();
		let data = path_data(&contours, min.to_vec2());
		let _ = writeln!(
			svg,
			"\t<path d=\"{data}\" fill=\"#{r:02x}{g:02x}{b:02x}\"/>"
		);
	}
	svg.push_str("</svg>");
	Some(svg)
}

fn translate(contours: &[Vec<Segment>], offset: Vec2) -> Vec<Vec<Segment>> {
	contours
		.iter()
		.map(|contour| {
			contour
				.iter()
				.map(|segment| segment.translate(offset))
				.collect()
		})
		.collect()
}

fn hull(contours: &[Vec<Segment>]) -> Rect {
	contours
		.iter()
		.flatten()
		.map(Segment::hull)
		.fold(Rect::NOTHING, Rect::union)
}

fn winding(contours: &[Vec<Segment>], pos: Pos2) -> i32 {
	contours
		.iter()
		.flatten()
		.map(|segment| segment.winding(pos))
		.sum()
}

fn subtract(contours: &[Vec<Segment>], cutters: &[Vec<Segment>]) -> Vec<Vec<Segment>> {
	let segments: Vec<Segment> = contours.iter().chain(cutters).flatten().copied().collect();
	let mut splits = vec![Vec::new(); segments.len()];
	for (i, &a) in segments.iter().enumerate() {
		for (j, &b) in segments.iter().enumerate().skip(i + 1) {
			let mut hits = Vec::new();
			intersect(a, (0.0, 1.0), b, (0.0, 1.0), 0, &mut hits);
			for (s, u, pos) in hits {
				splits[i].push((s, pos));
				splits[j].push((u, pos));
			}
		}
	}

	let inside = |pos: Pos2| winding(contours, pos) != 0 && winding(cutters, pos) == 0;
	let mut kept: Vec<Segment> = Vec::new();
	for (segment, splits) in segments.iter().zip(splits) {
		for piece in segment.pieces(splits) {
			let (pos, normal) = piece.sample();
			let left = inside(pos + normal * SAMPLE_OFFSET);
			if left == inside(pos - normal * SAMPLE_OFFSET) {
				continue;
			}

			let piece = if left { piece } else { piece.reversed() };
			if !kept.iter().any(|other| other.matches(&piece)) {
				kept.push(piece);
			}
		}
	}

	chain(kept)
}

fn intersect(
	a: Segment,
	range_a: (f32, f32),
	b: Segment,
	range_b: (f32, f32),
	depth: u32,
	hits: &mut Vec<(f32, f32, Pos2)>,
) {
	if !a.hull().expand(SNAP).intersects(b.hull()) {
		return;
	}

	let (flat_a, flat_b) = (a.is_flat(), b.is_flat());
	if (flat_a && flat_b) || depth == MAX_DEPTH {
		let lerp = |(from, to): (f32, f32), t: f32| from + (to - from) * t;
		for (s, u, pos) in line_hits(a.start(), a.end(), b.start(), b.end()) {
			hits.push((lerp(range_a, s), lerp(range_b, u), pos));
		}
		return;
	}

	let size = |segment: Segment| segment.hull().size().length();
	if !flat_a && (flat_b || size(a) >= size(b)) {
		let (left, right) = a.split(0.5);
		let middle = (range_a.0 + range_a.1) / 2.0;
		intersect(left, (range_a.0, middle), b, range_b, depth + 1, hits);
		intersect(right, (middle, range_a.1), b, range_b, depth + 1, hits);
	} else {
		let (left, right) = b.split(0.5);
		let middle = (range_b.0 + range_b.1) / 2.0;
		intersect(a, range_a, left, (range_b.0, middle), depth + 1, hits);
		intersect(a, range_a, right, (middle, range_b.1), depth + 1, hits);
	}
}

fn line_hits(a0: Pos2, a1: Pos2, b0: Pos2, b1: Pos2) -> Vec<(f32, f32, Pos2)> {
	let (d, e) = (a1 - a0, b1 - b0);
	let (length_d, length_e) = (d.length(), e.length());
	if length_d <= f32::EPSILON || length_e <= f32::EPSILON {
		return Vec::new();
	}

	let within = |t: f32, length: f32| (-SNAP / length..=1.0 + SNAP / length).contains(&t);
	let offset = b0 - a0;
	let denominator = cross(d, e);
	if denominator.abs() > PARALLEL * length_d * length_e {
		let s = cross(offset, e) / denominator;
		let u = cross(offset, d) / denominator;
		if !within(s, length_d) || !within(u, length_e) {
			return Vec::new();
		}

		let s = s.clamp(0.0, 1.0);
		return vec![(s, u.clamp(0.0, 1.0), a0 + d * s)];
	}

	if cross(d, offset).abs() / length_d > SNAP {
		return Vec::new();
	}

	let project = |pos: Pos2, origin: Pos2, direction: Vec2| {
		(pos - origin).dot(direction) / direction.length_sq()
	};
	let mut hits = Vec::new();
	for (u, pos) in [(0.0, b0), (1.0, b1)] {
		let s = project(pos, a0, d);
		if within(s, length_d) {
			hits.push((s.clamp(0.0, 1.0), u, pos));
		}
	}
	for (s, pos) in [(0.0, a0), (1.0, a1)] {
		let u = project(pos, b0, e);
		if within(u, length_e) {
			hits.push((s, u.clamp(0.0, 1.0), pos));
		}
	}
	hits
}

fn chain(mut pieces: Vec<Segment>) -> Vec<Vec<Segment>> {
	let mut points: Vec<Pos2> = Vec::new();
	let mut snap = |pos: Pos2| {
		if let Some(&found) = points.iter().find(|point| point.distance(pos) <= SNAP) {
			return found;
		}

		points.push(pos);
		pos
	};
	for piece in &mut pieces {
		let (start, end) = (snap(piece.start()), snap(piece.end()));
		piece.set_start(start);
		piece.set_end(end);
	}
	pieces.retain(|piece| piece.start() != piece.end());

	let mut used = vec![false; pieces.len()];
	let mut contours = Vec::new();
	for first in 0..pieces.len() {
		if used[first] {
			continue;
		}

		let start = pieces[first].start();
		let mut contour = Vec::new();
		let mut next = Some(first);
		while let Some(index) = next {
			used[index] = true;
			contour.push(pieces[index]);
			let end = pieces[index].end();
			next = (0..pieces.len())
				.find(|&other| !used[other] && pieces[other].start() == end)
				.filter(|_| end != start);
		}
		contours.push(merge_lines(contour));
	}
	contours
}

fn merge_lines(contour: Vec<Segment>) -> Vec<Segment> {
	let mut merged: Vec<Segment> = Vec::new();
	for segment in contour {
		if let Some(last) = merged.last_mut()
			&& let Some(line) = join(last, &segment)
		{
			*last = line;
		} else {
			merged.push(segment);
		}
	}

	if merged.len() > 2
		&& let Some(line) = join(&merged[merged.len() - 1], &merged[0])
	{
		merged[0] = line;
		merged.pop();
	}
	merged
}

fn join(a: &Segment, b: &Segment) -> Option<Segment> {
	let (Segment::Line([start, middle]), Segment::Line([_, end])) = (*a, *b) else {
		return None;
	};

	let chord = end - start;
	let straight = cross(chord, middle - start).abs() <= FLATNESS * chord.length();
	let forward = (middle - start).dot(end - middle) > 0.0;
	(straight && forward).then_some(Segment::Line([start, end]))
}

fn path_data(contours: &[Vec<Segment>], origin: Vec2) -> String {
	let point = |pos: Pos2| {
		let pos = pos - origin;
		[number(pos.x), number(pos.y)]
	};

	let mut data = String::new();
	for contour in contours {
		let mut last = point(contour[0].start());
		let _ = write!(data, "M{} {}", last[0], last[1]);
		for (index, segment) in contour.iter().enumerate() {
			match *segment {
				Segment::Line(_) if index + 1 == contour.len() => {}
				Segment::Line([_, end]) => {
					let [x, y] = point(end);
					match (x == last[0], y == last[1]) {
						(true, true) => {}
						(true, false) => {
							let _ = write!(data, "V{y}");
						}
						(false, true) => {
							let _ = write!(data, "H{x}");
						}
						(false, false) => {
							let _ = write!(data, "L{x} {y}");
						}
					}
					last = [x, y];
				}
				Segment::Cubic([_, a, b, end]) => {
					let ([ax, ay], [bx, by]) = (point(a), point(b));
					last = point(end);
					let _ = write!(data, "C{ax} {ay} {bx} {by} {} {}", last[0], last[1]);
				}
			}
		}
		data.push('Z');
	}
	data
}

fn number(value: f32) -> String {
	let text = format!("{value:.DECIMALS$}");
	let text = text.trim_end_matches('0').trim_end_matches('.');
	if text == "-0" { "0" } else { text }.to_string()
}
