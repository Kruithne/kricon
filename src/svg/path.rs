use super::Point;
use std::f32::consts::{FRAC_PI_2, TAU};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
	Move(Point),
	Line(Point),
	Quad(Point, Point),
	Cubic(Point, Point, Point),
	Close,
}

struct Cursor<'a> {
	bytes: &'a [u8],
	pos: usize,
}

impl Cursor<'_> {
	fn skip_separators(&mut self) {
		while self.pos < self.bytes.len()
			&& (self.bytes[self.pos].is_ascii_whitespace() || self.bytes[self.pos] == b',')
		{
			self.pos += 1;
		}
	}

	fn letter(&mut self) -> Option<u8> {
		self.skip_separators();
		let byte = *self.bytes.get(self.pos)?;
		if !byte.is_ascii_alphabetic() {
			return None;
		}

		self.pos += 1;
		Some(byte)
	}

	fn number(&mut self) -> Option<f32> {
		self.skip_separators();
		let start = self.pos;
		self.eat(|b| b == b'+' || b == b'-');
		let digits = self.digits();
		let fraction = self.eat(|b| b == b'.') && self.digits();
		if !digits && !fraction {
			self.pos = start;
			return None;
		}

		let mark = self.pos;
		if self.eat(|b| b == b'e' || b == b'E') {
			self.eat(|b| b == b'+' || b == b'-');
			if !self.digits() {
				self.pos = mark;
			}
		}

		std::str::from_utf8(&self.bytes[start..self.pos])
			.ok()?
			.parse()
			.ok()
	}

	fn flag(&mut self) -> Option<bool> {
		self.skip_separators();
		let byte = *self.bytes.get(self.pos)?;
		self.pos += 1;
		match byte {
			b'0' => Some(false),
			b'1' => Some(true),
			_ => None,
		}
	}

	fn point(&mut self) -> Option<Point> {
		Some(Point::new(self.number()?, self.number()?))
	}

	fn eat(&mut self, test: impl Fn(u8) -> bool) -> bool {
		let matched = self.bytes.get(self.pos).is_some_and(|&b| test(b));
		if matched {
			self.pos += 1;
		}

		matched
	}

	fn digits(&mut self) -> bool {
		let start = self.pos;
		while self.eat(|b| b.is_ascii_digit()) {}
		self.pos > start
	}

	fn done(&mut self) -> bool {
		self.skip_separators();
		self.pos >= self.bytes.len()
	}
}

pub fn parse(data: &str) -> Vec<Command> {
	let mut commands = Vec::new();
	let mut cursor = Cursor {
		bytes: data.as_bytes(),
		pos: 0,
	};
	let mut letter = 0;
	let mut current = Point::default();
	let mut start = Point::default();
	let mut previous = None;

	while !cursor.done() {
		if let Some(next) = cursor.letter() {
			letter = next;
		} else if letter == 0 {
			break;
		}

		let relative = letter.is_ascii_lowercase();
		let origin = if relative { current } else { Point::default() };
		let offset = |p: Point| Point::new(p.x + origin.x, p.y + origin.y);

		let command = match letter.to_ascii_uppercase() {
			b'M' => {
				let Some(p) = cursor.point() else {
					break;
				};

				letter = if relative { b'l' } else { b'L' };
				start = offset(p);
				Command::Move(start)
			}
			b'L' => match cursor.point() {
				Some(p) => Command::Line(offset(p)),
				None => break,
			},
			b'H' => match cursor.number() {
				Some(x) => Command::Line(Point::new(x + origin.x, current.y)),
				None => break,
			},
			b'V' => match cursor.number() {
				Some(y) => Command::Line(Point::new(current.x, y + origin.y)),
				None => break,
			},
			b'C' => match (cursor.point(), cursor.point(), cursor.point()) {
				(Some(a), Some(b), Some(p)) => Command::Cubic(offset(a), offset(b), offset(p)),
				_ => break,
			},
			b'S' => match (cursor.point(), cursor.point()) {
				(Some(b), Some(p)) => {
					let a = match previous {
						Some(Command::Cubic(_, c, _)) => reflect(c, current),
						_ => current,
					};

					Command::Cubic(a, offset(b), offset(p))
				}
				_ => break,
			},
			b'Q' => match (cursor.point(), cursor.point()) {
				(Some(a), Some(p)) => Command::Quad(offset(a), offset(p)),
				_ => break,
			},
			b'T' => match cursor.point() {
				Some(p) => {
					let a = match previous {
						Some(Command::Quad(c, _)) => reflect(c, current),
						_ => current,
					};

					Command::Quad(a, offset(p))
				}
				None => break,
			},
			b'A' => {
				let arc = (|| {
					Some((
						cursor.number()?,
						cursor.number()?,
						cursor.number()?,
						cursor.flag()?,
						cursor.flag()?,
						cursor.point()?,
					))
				})();
				let Some((rx, ry, angle, large, sweep, p)) = arc else {
					break;
				};

				let to = offset(p);
				arc_to_cubics(current, rx, ry, angle, large, sweep, to, &mut commands);
				current = to;
				previous = None;
				continue;
			}
			b'Z' => {
				letter = 0;
				Command::Close
			}
			_ => break,
		};

		current = match command {
			Command::Move(p) | Command::Line(p) | Command::Quad(_, p) | Command::Cubic(_, _, p) => {
				p
			}
			Command::Close => start,
		};

		previous = Some(command);
		commands.push(command);
	}

	commands
}

fn reflect(control: Point, about: Point) -> Point {
	Point::new(2.0 * about.x - control.x, 2.0 * about.y - control.y)
}

#[allow(clippy::too_many_arguments)]
fn arc_to_cubics(
	from: Point,
	rx: f32,
	ry: f32,
	angle: f32,
	large: bool,
	sweep: bool,
	to: Point,
	out: &mut Vec<Command>,
) {
	if from == to {
		return;
	}

	let (mut rx, mut ry) = (rx.abs(), ry.abs());
	if rx == 0.0 || ry == 0.0 {
		out.push(Command::Line(to));
		return;
	}

	let (sin, cos) = angle.to_radians().sin_cos();
	let dx = (from.x - to.x) / 2.0;
	let dy = (from.y - to.y) / 2.0;
	let x1 = cos * dx + sin * dy;
	let y1 = -sin * dx + cos * dy;

	let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
	if lambda > 1.0 {
		rx *= lambda.sqrt();
		ry *= lambda.sqrt();
	}

	let numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
	let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
	let mut coefficient = (numerator / denominator).max(0.0).sqrt();
	if large == sweep {
		coefficient = -coefficient;
	}

	let cx1 = coefficient * rx * y1 / ry;
	let cy1 = -coefficient * ry * x1 / rx;
	let cx = cos * cx1 - sin * cy1 + (from.x + to.x) / 2.0;
	let cy = sin * cx1 + cos * cy1 + (from.y + to.y) / 2.0;

	let theta = ((y1 - cy1) / ry).atan2((x1 - cx1) / rx);
	let mut delta = ((-y1 - cy1) / ry).atan2((-x1 - cx1) / rx) - theta;
	if sweep && delta < 0.0 {
		delta += TAU;
	} else if !sweep && delta > 0.0 {
		delta -= TAU;
	}

	let count = (delta.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
	let step = delta / count as f32;
	let k = 4.0 / 3.0 * (step / 4.0).tan();
	let map = |ux: f32, uy: f32| {
		Point::new(
			cx + rx * cos * ux - ry * sin * uy,
			cy + rx * sin * ux + ry * cos * uy,
		)
	};

	for i in 0..count {
		let (s1, c1) = (theta + step * i as f32).sin_cos();
		let (s2, c2) = (theta + step * (i + 1) as f32).sin_cos();
		let end = if i + 1 == count { to } else { map(c2, s2) };
		out.push(Command::Cubic(
			map(c1 - k * s1, s1 + k * c1),
			map(c2 + k * s2, s2 - k * c2),
			end,
		));
	}
}
