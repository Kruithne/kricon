use crate::mesh::Mesh;
use crate::view::View;
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

const WARNING_COLOR: Color32 = Color32::from_rgb(255, 150, 30);
const STROKE_WIDTH: f32 = 2.0;
const HEAD_SIZE: f32 = 6.0;
const EPSILON: f32 = 1e-4;
const CONE_MARGIN: f32 = 0.05;
const PARALLEL: f32 = 0.17;

#[derive(Default)]
pub struct Spacing {
	pub distance: f32,
	boundary: Vec<[Pos2; 4]>,
	measured: f32,
	arrows: Vec<[Pos2; 2]>,
}

impl Spacing {
	pub fn update(&mut self, mesh: &Mesh) {
		if self.distance <= 0.0 {
			self.arrows.clear();
			self.boundary.clear();
			return;
		}

		let boundary = mesh.boundary();
		if boundary == self.boundary && self.distance == self.measured {
			return;
		}

		self.arrows = measure(&boundary, self.distance);
		self.boundary = boundary;
		self.measured = self.distance;
	}

	pub fn draw(&self, view: &View, painter: &egui::Painter) {
		let stroke = egui::Stroke::new(STROKE_WIDTH, WARNING_COLOR);
		for &[a, b] in &self.arrows {
			let (a, b) = (view.to_screen(a), view.to_screen(b));
			painter.line_segment([a, b], stroke);
			draw_head(painter, a, b, stroke);
			draw_head(painter, b, a, stroke);
		}
	}
}

fn draw_head(painter: &egui::Painter, tip: Pos2, from: Pos2, stroke: egui::Stroke) {
	let back = (from - tip).normalized() * HEAD_SIZE;
	let side = back.rot90() * 0.5;
	painter.line_segment([tip, tip + back + side], stroke);
	painter.line_segment([tip, tip + back - side], stroke);
}

fn measure(segments: &[[Pos2; 4]], limit: f32) -> Vec<[Pos2; 2]> {
	let mut bounds: Vec<(Rect, usize)> = segments
		.iter()
		.enumerate()
		.map(|(index, s)| (Rect::from_two_pos(s[1], s[2]), index))
		.collect();
	bounds.sort_by(|a, b| a.0.min.x.total_cmp(&b.0.min.x));

	let mut arrows: Vec<[Pos2; 2]> = Vec::new();
	for (index, (rect, a)) in bounds.iter().enumerate() {
		let reach = rect.expand(limit);
		for (other, b) in &bounds[index + 1..] {
			if other.min.x > reach.max.x {
				break;
			}

			if !reach.intersects(*other) {
				continue;
			}

			let Some([p, q]) = closest(&segments[*a], &segments[*b], limit) else {
				continue;
			};

			let middle = p.lerp(q, 0.5);
			let near = |&[x, y]: &[Pos2; 2]| x.lerp(y, 0.5).distance(middle) < limit / 2.0;
			if !arrows.iter().any(near) {
				arrows.push([p, q]);
			}
		}
	}
	arrows
}

fn closest(s1: &[Pos2; 4], s2: &[Pos2; 4], limit: f32) -> Option<[Pos2; 2]> {
	if intersects(s1[1], s1[2], s2[1], s2[2]) {
		return None;
	}

	let (u1, u2) = ((s1[2] - s1[1]).normalized(), (s2[2] - s2[1]).normalized());
	if cross(u1, u2).abs() < PARALLEL {
		return parallel(s1, s2, u1, u2, limit);
	}

	[
		(s1[1], project(s1[1], s2[1], s2[2])),
		(s1[2], project(s1[2], s2[1], s2[2])),
		(project(s2[1], s1[1], s1[2]), s2[1]),
		(project(s2[2], s1[1], s1[2]), s2[2]),
	]
	.into_iter()
	.filter(|&(p, q)| {
		let distance = p.distance(q);
		let direction = (q - p) / distance;
		distance > EPSILON
			&& distance < limit
			&& local_minimum(s1, p, direction)
			&& local_minimum(s2, q, -direction)
	})
	.min_by(|a, b| a.0.distance(a.1).total_cmp(&b.0.distance(b.1)))
	.map(|(p, q)| [p, q])
}

fn parallel(s1: &[Pos2; 4], s2: &[Pos2; 4], u1: Vec2, u2: Vec2, limit: f32) -> Option<[Pos2; 2]> {
	let (c, d) = ((s2[1] - s1[1]).dot(u1), (s2[2] - s1[1]).dot(u1));
	let low = c.min(d).max(0.0);
	let high = c.max(d).min((s1[2] - s1[1]).length());
	if high - low < EPSILON {
		return None;
	}

	let p = s1[1] + u1 * (low + high) / 2.0;
	let q = project(p, s2[1], s2[2]);
	let distance = p.distance(q);
	let facing = cross(u1, q - p) * cross(u2, q - p) < 0.0;
	(distance > EPSILON && distance < limit && facing).then_some([p, q])
}

fn local_minimum(s: &[Pos2; 4], point: Pos2, direction: Vec2) -> bool {
	let (incoming, outgoing) = if point.distance(s[1]) < EPSILON {
		(s[1] - s[0], s[2] - s[1])
	} else if point.distance(s[2]) < EPSILON {
		(s[2] - s[1], s[3] - s[2])
	} else {
		return true;
	};

	direction.dot(incoming.normalized()) > CONE_MARGIN
		&& direction.dot(outgoing.normalized()) < -CONE_MARGIN
}

fn cross(a: Vec2, b: Vec2) -> f32 {
	a.x * b.y - a.y * b.x
}

fn project(point: Pos2, a: Pos2, b: Pos2) -> Pos2 {
	let edge = b - a;
	let length = edge.length_sq();
	if length < EPSILON {
		return a;
	}

	a + edge * ((point - a).dot(edge) / length).clamp(0.0, 1.0)
}

fn intersects(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
	let side = |p: Pos2, q: Pos2, r: Pos2| cross(q - p, r - p);
	let (d1, d2) = (side(c, d, a), side(c, d, b));
	let (d3, d4) = (side(a, b, c), side(a, b, d));
	(d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}
