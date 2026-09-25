use eframe::egui::{Pos2, Rect, Vec2};
use std::f32::consts::TAU;

#[derive(Clone, Copy)]
pub enum Segment {
	Line([Pos2; 2]),
	Cubic([Pos2; 4]),
}

impl Segment {
	pub fn points(&self) -> &[Pos2] {
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

	pub fn start(&self) -> Pos2 {
		self.points()[0]
	}

	pub fn end(&self) -> Pos2 {
		self.points()[self.points().len() - 1]
	}

	pub fn set_start(&mut self, pos: Pos2) {
		self.points_mut()[0] = pos;
	}

	pub fn set_end(&mut self, pos: Pos2) {
		let points = self.points_mut();
		points[points.len() - 1] = pos;
	}

	pub fn translate(mut self, offset: Vec2) -> Self {
		for point in self.points_mut() {
			*point += offset;
		}
		self
	}

	pub fn reversed(self) -> Self {
		match self {
			Self::Line([a, b]) => Self::Line([b, a]),
			Self::Cubic([a, b, c, d]) => Self::Cubic([d, c, b, a]),
		}
	}

	pub fn split(&self, t: f32) -> (Self, Self) {
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

	pub fn point(&self, t: f32) -> Pos2 {
		self.split(t).0.end()
	}

	pub fn hull(&self) -> Rect {
		Rect::from_points(self.points())
	}

	pub fn bounds(&self) -> Rect {
		let mut params = vec![0.0, 1.0];
		params.extend(self.extrema(0));
		params.extend(self.extrema(1));
		let points: Vec<Pos2> = params.into_iter().map(|t| self.point(t)).collect();
		Rect::from_points(&points)
	}

	pub fn extrema(&self, axis: usize) -> Vec<f32> {
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
}

pub fn cross(a: Vec2, b: Vec2) -> f32 {
	a.x * b.y - a.y * b.x
}

pub fn turn(from: Vec2, to: Vec2) -> f32 {
	cross(from, to).atan2(from.dot(to)).rem_euclid(TAU)
}

pub fn area(polygon: &[Pos2]) -> f32 {
	let mut area = 0.0;
	for (index, &a) in polygon.iter().enumerate() {
		let b = polygon[(index + 1) % polygon.len()];
		area += cross(a.to_vec2(), b.to_vec2());
	}
	area / 2.0
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
