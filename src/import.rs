use crate::mesh::{area, encloses};
use crate::svg::{self, Point};
use eframe::egui::{Color32, Pos2, Vec2, pos2};

const TOLERANCE: f32 = 0.02;
const SPLINE_TOLERANCE: f32 = 0.005;
const MIN_SPACING: f32 = 1e-4;
const SMOOTH: f32 = 0.995;
const MIN_FIT_POINTS: usize = 4;
const MAX_FIT_POINTS: usize = 512;
const RELAX_STEPS: usize = 64;
const MAX_DEPTH: u32 = 10;
const IDENTITY: Transform = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
const SKIPPED: [&str; 8] = [
	"defs",
	"clipPath",
	"mask",
	"symbol",
	"pattern",
	"marker",
	"linearGradient",
	"radialGradient",
];

type Transform = [f32; 6];

pub struct Shape {
	pub points: Vec<Pos2>,
	pub curve: bool,
	pub holdout: bool,
	pub color: Option<Color32>,
}

#[derive(Clone, Copy)]
enum Paint {
	None,
	Default,
	Color(Color32),
}

#[derive(Clone, Copy)]
struct Style {
	transform: Transform,
	fill: Paint,
	even_odd: bool,
}

#[derive(Clone, Copy)]
enum Segment {
	Line([Pos2; 2]),
	Cubic([Pos2; 4]),
}

impl Segment {
	fn points(&self) -> &[Pos2] {
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

	fn start_tangent(&self) -> Vec2 {
		let points = self.points();
		points[1..]
			.iter()
			.map(|&point| point - points[0])
			.find(|tangent| tangent.length() > MIN_SPACING)
			.unwrap_or_default()
	}

	fn end_tangent(&self) -> Vec2 {
		let end = self.end();
		self.points()
			.iter()
			.rev()
			.map(|&point| end - point)
			.find(|tangent| tangent.length() > MIN_SPACING)
			.unwrap_or_default()
	}

	fn is_degenerate(&self) -> bool {
		self.points()
			.iter()
			.all(|point| point.distance(self.start()) <= MIN_SPACING)
	}
}

pub fn parse(text: &str) -> Vec<Shape> {
	let mut shapes = Vec::new();
	let mut stack = vec![Style {
		transform: IDENTITY,
		fill: Paint::Default,
		even_odd: false,
	}];
	let mut skipped = 0usize;

	for tag in svg::tags(text) {
		let closing = tag.starts_with('/');
		let empty = tag.ends_with('/');
		let name = tag
			.trim_start_matches('/')
			.split(|c: char| c.is_whitespace() || c == '/')
			.next()
			.unwrap_or("");

		if SKIPPED.contains(&name) {
			if closing {
				skipped = skipped.saturating_sub(1);
			} else if !empty {
				skipped += 1;
			}
			continue;
		}

		if skipped > 0 {
			continue;
		}

		let group = name == "g" || name == "svg";
		if closing {
			if group && stack.len() > 1 {
				stack.pop();
			}
			continue;
		}

		let style = inherit(stack[stack.len() - 1], tag);
		if group {
			if !empty {
				stack.push(style);
			}
			continue;
		}

		let color = match style.fill {
			Paint::None => continue,
			Paint::Default => None,
			Paint::Color(color) => Some(color),
		};
		if let Some(data) = shape_data(name, tag) {
			shapes.extend(convert(&data, style, color));
		}
	}

	shapes
}

fn inherit(parent: Style, tag: &str) -> Style {
	Style {
		transform: svg::attribute(tag, "transform").map_or(parent.transform, |value| {
			multiply(parent.transform, parse_transform(value))
		}),
		fill: property(tag, "fill").map_or(parent.fill, parse_paint),
		even_odd: property(tag, "fill-rule").map_or(parent.even_odd, |rule| rule == "evenodd"),
	}
}

fn property<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
	svg::attribute(tag, "style")
		.and_then(|style| {
			style.split(';').find_map(|declaration| {
				let (key, value) = declaration.split_once(':')?;
				(key.trim() == name).then_some(value)
			})
		})
		.or_else(|| svg::attribute(tag, name))
		.map(str::trim)
}

fn parse_paint(value: &str) -> Paint {
	match value {
		"none" | "transparent" => Paint::None,
		"black" => Paint::Color(Color32::BLACK),
		"white" => Paint::Color(Color32::WHITE),
		_ => Color32::from_hex(value).map_or(Paint::Default, Paint::Color),
	}
}

fn shape_data(name: &str, tag: &str) -> Option<String> {
	let value = |key: &str| {
		svg::attribute(tag, key).and_then(|value| value.trim().trim_end_matches("px").parse().ok())
	};
	let number = |key: &str| value(key).unwrap_or(0.0);

	match name {
		"path" => svg::attribute(tag, "d").map(str::to_string),
		"polygon" | "polyline" => svg::attribute(tag, "points").map(|points| format!("M{points}Z")),
		"circle" => ellipse(number("cx"), number("cy"), number("r"), number("r")),
		"ellipse" => ellipse(number("cx"), number("cy"), number("rx"), number("ry")),
		"rect" => {
			let (x, y, width, height) =
				(number("x"), number("y"), number("width"), number("height"));
			if width <= 0.0 || height <= 0.0 {
				return None;
			}

			let (rx, ry) = match (value("rx"), value("ry")) {
				(Some(rx), Some(ry)) => (rx, ry),
				(Some(radius), None) | (None, Some(radius)) => (radius, radius),
				(None, None) => (0.0, 0.0),
			};
			let (rx, ry) = (rx.clamp(0.0, width / 2.0), ry.clamp(0.0, height / 2.0));
			let (right, bottom) = (x + width, y + height);
			let arc = format!("A{rx} {ry} 0 0 1");
			Some(format!(
				"M{} {y}H{}{arc} {right} {}V{}{arc} {} {bottom}H{}{arc} {x} {}V{}{arc} {} {y}Z",
				x + rx,
				right - rx,
				y + ry,
				bottom - ry,
				right - rx,
				x + rx,
				bottom - ry,
				y + ry,
				x + rx,
			))
		}
		_ => None,
	}
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<String> {
	if rx <= 0.0 || ry <= 0.0 {
		return None;
	}

	let arc = format!("A{rx} {ry} 0 1 0");
	Some(format!(
		"M{} {cy}{arc} {} {cy}{arc} {} {cy}Z",
		cx - rx,
		cx + rx,
		cx - rx
	))
}

fn convert(data: &str, style: Style, color: Option<Color32>) -> Vec<Shape> {
	let contours = contours(&svg::parse_path(data), style.transform);
	let outlines: Vec<Vec<Pos2>> = contours.iter().map(|contour| flatten(contour)).collect();
	let orientation = |outline: &[Pos2]| area(outline).signum() as i32;

	let mut shapes: Vec<(usize, Shape)> = Vec::new();
	for (index, contour) in contours.iter().enumerate() {
		let outline = &outlines[index];
		let parents: Vec<&Vec<Pos2>> = outlines
			.iter()
			.enumerate()
			.filter(|&(other, parent)| other != index && encloses(parent, outline[0]))
			.map(|(_, parent)| parent)
			.collect();
		let winding: i32 = orientation(outline)
			+ parents
				.iter()
				.map(|parent| orientation(parent))
				.sum::<i32>();
		let holdout = if style.even_odd {
			parents.len() % 2 == 1
		} else {
			winding == 0
		};

		let (points, curve) = shape_points(contour, outline);
		if points.len() < 3 {
			continue;
		}

		shapes.push((
			parents.len(),
			Shape {
				points,
				curve,
				holdout,
				color: color.filter(|_| !holdout),
			},
		));
	}

	shapes.sort_by_key(|&(depth, _)| depth);
	shapes.into_iter().map(|(_, shape)| shape).collect()
}

fn contours(path: &[svg::Segment], transform: Transform) -> Vec<Vec<Segment>> {
	let pos = |point: Point| {
		let [a, b, c, d, e, f] = transform;
		pos2(a * point.x + c * point.y + e, b * point.x + d * point.y + f)
	};

	let mut contours = Vec::new();
	let mut contour = Vec::new();
	let (mut start, mut current) = (Pos2::ZERO, Pos2::ZERO);
	for &segment in path {
		let segment = match segment {
			svg::Segment::Move(point) => {
				close(&mut contours, &mut contour, start, current);
				start = pos(point);
				current = start;
				continue;
			}
			svg::Segment::Close => {
				close(&mut contours, &mut contour, start, current);
				current = start;
				continue;
			}
			svg::Segment::Line(point) => Segment::Line([current, pos(point)]),
			svg::Segment::Quad(control, point) => {
				let (control, end) = (pos(control), pos(point));
				Segment::Cubic([
					current,
					current + (control - current) * (2.0 / 3.0),
					end + (control - end) * (2.0 / 3.0),
					end,
				])
			}
			svg::Segment::Cubic(a, b, point) => {
				Segment::Cubic([current, pos(a), pos(b), pos(point)])
			}
		};

		current = segment.end();
		contour.push(segment);
	}

	close(&mut contours, &mut contour, start, current);
	contours
}

fn close(contours: &mut Vec<Vec<Segment>>, contour: &mut Vec<Segment>, start: Pos2, current: Pos2) {
	if current.distance(start) > TOLERANCE {
		contour.push(Segment::Line([current, start]));
	}
	contour.retain(|segment| !segment.is_degenerate());
	if contour.len() >= 2 {
		contours.push(std::mem::take(contour));
	}
	contour.clear();
}

fn shape_points(contour: &[Segment], outline: &[Pos2]) -> (Vec<Pos2>, bool) {
	let cubics: Option<Vec<[Pos2; 4]>> = contour
		.iter()
		.map(|segment| match *segment {
			Segment::Cubic(points) => Some(points),
			Segment::Line(_) => None,
		})
		.collect();

	let Some(cubics) = cubics else {
		return (dedup(outline), false);
	};

	if let Some(points) = spline(&cubics) {
		return (points, true);
	}

	if is_smooth(contour)
		&& let Some(points) = fit(outline)
	{
		return (points, true);
	}

	(dedup(outline), false)
}

fn spline(cubics: &[[Pos2; 4]]) -> Option<Vec<Pos2>> {
	let count = cubics.len();
	if count < 3 {
		return None;
	}

	let points: Vec<Pos2> = cubics.iter().map(|&[_, b, c, _]| b + (b - c)).collect();
	let point = |index: usize| points[(index + count - 1) % count].to_vec2();
	let exact = cubics.iter().enumerate().all(|(index, cubic)| {
		let [a, b, c, d] = [0, 1, 2, 3].map(|offset| point(index + offset));
		let span = [
			(a + b * 4.0 + c) / 6.0,
			(b * 2.0 + c) / 3.0,
			(b + c * 2.0) / 3.0,
			(b + c * 4.0 + d) / 6.0,
		];
		span.iter()
			.zip(cubic)
			.all(|(expected, actual)| expected.to_pos2().distance(*actual) <= SPLINE_TOLERANCE)
	});
	exact.then_some(points)
}

fn is_smooth(contour: &[Segment]) -> bool {
	contour.iter().enumerate().all(|(index, segment)| {
		let next = contour[(index + 1) % contour.len()];
		segment
			.end_tangent()
			.normalized()
			.dot(next.start_tangent().normalized())
			>= SMOOTH
	})
}

fn fit(outline: &[Pos2]) -> Option<Vec<Pos2>> {
	let mut count = MIN_FIT_POINTS;
	while count <= MAX_FIT_POINTS {
		let samples = resample(outline, count * 2);
		let targets: Vec<Pos2> = samples.iter().step_by(2).copied().collect();
		let points = interpolate(&targets);

		let point = |index: usize| points[index % count].to_vec2();
		let within = (0..count).all(|index| {
			let [a, b, c, d] = [0, 1, 2, 3].map(|offset| point(index + count - 1 + offset));
			let middle = (a + (b + c) * 23.0 + d) / 48.0;
			middle.to_pos2().distance(samples[index * 2 + 1]) <= TOLERANCE
		});
		if within {
			return Some(points);
		}

		count *= 2;
	}

	None
}

fn interpolate(targets: &[Pos2]) -> Vec<Pos2> {
	let count = targets.len();
	let mut points = targets.to_vec();
	for _ in 0..RELAX_STEPS {
		for index in 0..count {
			let prev = points[(index + count - 1) % count].to_vec2();
			let next = points[(index + 1) % count].to_vec2();
			points[index] = ((targets[index].to_vec2() * 6.0 - prev - next) / 4.0).to_pos2();
		}
	}
	points
}

fn resample(outline: &[Pos2], count: usize) -> Vec<Pos2> {
	let closed: Vec<Pos2> = outline.iter().chain(outline.first()).copied().collect();
	let lengths: Vec<f32> = closed
		.windows(2)
		.map(|pair| pair[0].distance(pair[1]))
		.collect();
	let step = lengths.iter().sum::<f32>() / count as f32;

	let mut samples = Vec::with_capacity(count);
	let (mut segment, mut start) = (0, 0.0);
	for index in 0..count {
		let target = step * index as f32;
		while segment < lengths.len() - 1 && start + lengths[segment] < target {
			start += lengths[segment];
			segment += 1;
		}

		let t = if lengths[segment] > 0.0 {
			((target - start) / lengths[segment]).min(1.0)
		} else {
			0.0
		};
		samples.push(closed[segment].lerp(closed[segment + 1], t));
	}
	samples
}

fn flatten(contour: &[Segment]) -> Vec<Pos2> {
	let mut outline = Vec::new();
	for segment in contour {
		outline.push(segment.start());
		if let Segment::Cubic(points) = *segment {
			subdivide(points, 0, &mut outline);
		}
	}
	outline
}

fn subdivide([a, b, c, d]: [Pos2; 4], depth: u32, outline: &mut Vec<Pos2>) {
	let chord = d - a;
	let length = chord.length();
	let distance = |p: Pos2| {
		if length > f32::EPSILON {
			(chord.x * (p - a).y - chord.y * (p - a).x).abs() / length
		} else {
			p.distance(a)
		}
	};
	if distance(b).max(distance(c)) <= TOLERANCE || depth == MAX_DEPTH {
		return;
	}

	let (ab, bc, cd) = (a.lerp(b, 0.5), b.lerp(c, 0.5), c.lerp(d, 0.5));
	let (abc, bcd) = (ab.lerp(bc, 0.5), bc.lerp(cd, 0.5));
	let middle = abc.lerp(bcd, 0.5);
	subdivide([a, ab, abc, middle], depth + 1, outline);
	outline.push(middle);
	subdivide([middle, bcd, cd, d], depth + 1, outline);
}

fn dedup(outline: &[Pos2]) -> Vec<Pos2> {
	let mut points: Vec<Pos2> = Vec::with_capacity(outline.len());
	for &point in outline {
		if points
			.last()
			.is_none_or(|last| last.distance(point) > MIN_SPACING)
		{
			points.push(point);
		}
	}

	while points.len() > 1 && points[0].distance(points[points.len() - 1]) <= MIN_SPACING {
		points.pop();
	}
	points
}

fn parse_transform(text: &str) -> Transform {
	text.split(')')
		.filter_map(|part| {
			let (name, args) = part.split_once('(')?;
			let name = name.trim_matches(|c: char| c.is_whitespace() || c == ',');
			let args: Vec<f32> = args
				.split(|c: char| c.is_whitespace() || c == ',')
				.filter_map(|arg| arg.parse().ok())
				.collect();

			Some(match (name, args.as_slice()) {
				("matrix", &[a, b, c, d, e, f]) => [a, b, c, d, e, f],
				("translate", &[x]) => [1.0, 0.0, 0.0, 1.0, x, 0.0],
				("translate", &[x, y]) => [1.0, 0.0, 0.0, 1.0, x, y],
				("scale", &[scale]) => [scale, 0.0, 0.0, scale, 0.0, 0.0],
				("scale", &[x, y]) => [x, 0.0, 0.0, y, 0.0, 0.0],
				("rotate", &[angle]) => rotation(angle),
				("rotate", &[angle, x, y]) => multiply(
					multiply([1.0, 0.0, 0.0, 1.0, x, y], rotation(angle)),
					[1.0, 0.0, 0.0, 1.0, -x, -y],
				),
				("skewX", &[angle]) => [1.0, 0.0, angle.to_radians().tan(), 1.0, 0.0, 0.0],
				("skewY", &[angle]) => [1.0, angle.to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
				_ => IDENTITY,
			})
		})
		.fold(IDENTITY, multiply)
}

fn rotation(angle: f32) -> Transform {
	let (sin, cos) = angle.to_radians().sin_cos();
	[cos, sin, -sin, cos, 0.0, 0.0]
}

fn multiply([a, b, c, d, e, f]: Transform, [g, h, i, j, k, l]: Transform) -> Transform {
	[
		a * g + c * h,
		b * g + d * h,
		a * i + c * j,
		b * i + d * j,
		a * k + c * l + e,
		b * k + d * l + f,
	]
}
