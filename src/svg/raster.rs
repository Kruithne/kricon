use super::{Command, FillRule, Svg};
use eframe::egui::{Pos2, pos2};

const SUBSAMPLES: usize = 16;
const TOLERANCE: f32 = 0.1;

struct Edge {
	from: Pos2,
	to: Pos2,
}

pub fn rasterize(svg: &Svg, width: usize, height: usize) -> Vec<u8> {
	let [vx, vy, vw, vh] = svg.view_box;
	let scale = (width as f32 / vw).min(height as f32 / vh);
	let dx = (width as f32 - vw * scale) / 2.0 - vx * scale;
	let dy = (height as f32 - vh * scale) / 2.0 - vy * scale;
	let transform = |p: Pos2| pos2(p.x * scale + dx, p.y * scale + dy);

	let mut alpha = vec![0.0f32; width * height];
	let mut coverage = vec![0.0f32; width * height];

	for path in &svg.paths {
		let edges = flatten(&path.commands, transform);
		coverage.fill(0.0);
		fill(&edges, path.fill_rule, width, height, &mut coverage);

		for (a, c) in alpha.iter_mut().zip(&coverage) {
			let c = c.min(1.0);
			*a += c * (1.0 - *a);
		}
	}

	alpha.iter().map(|a| (a * 255.0).round() as u8).collect()
}

fn flatten(commands: &[Command], transform: impl Fn(Pos2) -> Pos2) -> Vec<Edge> {
	let mut edges = Vec::new();
	let mut start = Pos2::ZERO;
	let mut current = Pos2::ZERO;
	let line_to = |edges: &mut Vec<Edge>, current: &mut Pos2, to: Pos2| {
		if *current != to {
			edges.push(Edge { from: *current, to });
		}

		*current = to;
	};

	for command in commands {
		match *command {
			Command::Move(p) => {
				line_to(&mut edges, &mut current, start);
				start = transform(p);
				current = start;
			}
			Command::Line(p) => line_to(&mut edges, &mut current, transform(p)),
			Command::Quad(a, p) => {
				let (p0, p1, p2) = (current, transform(a), transform(p));
				let count = steps(0.25, deviation(p0, p1, p2));
				for i in 1..=count {
					let t = i as f32 / count as f32;
					let u = 1.0 - t;
					let x = u * u * p0.x + 2.0 * u * t * p1.x + t * t * p2.x;
					let y = u * u * p0.y + 2.0 * u * t * p1.y + t * t * p2.y;
					line_to(&mut edges, &mut current, pos2(x, y));
				}
			}
			Command::Cubic(a, b, p) => {
				let (p0, p1, p2, p3) = (current, transform(a), transform(b), transform(p));
				let count = steps(0.75, deviation(p0, p1, p2).max(deviation(p1, p2, p3)));
				for i in 1..=count {
					let t = i as f32 / count as f32;
					let u = 1.0 - t;
					let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
					let x = w0 * p0.x + w1 * p1.x + w2 * p2.x + w3 * p3.x;
					let y = w0 * p0.y + w1 * p1.y + w2 * p2.y + w3 * p3.y;
					line_to(&mut edges, &mut current, pos2(x, y));
				}
			}
			Command::Close => line_to(&mut edges, &mut current, start),
		}
	}

	line_to(&mut edges, &mut current, start);
	edges
}

fn deviation(a: Pos2, b: Pos2, c: Pos2) -> f32 {
	let x = a.x - 2.0 * b.x + c.x;
	let y = a.y - 2.0 * b.y + c.y;
	(x * x + y * y).sqrt()
}

fn steps(factor: f32, deviation: f32) -> usize {
	((factor * deviation / TOLERANCE).sqrt().ceil() as usize).clamp(1, 256)
}

fn fill(edges: &[Edge], fill_rule: FillRule, width: usize, height: usize, coverage: &mut [f32]) {
	let weight = 1.0 / SUBSAMPLES as f32;
	let mut crossings: Vec<(f32, i32)> = Vec::new();

	for row in 0..height {
		let line = &mut coverage[row * width..(row + 1) * width];
		for sample in 0..SUBSAMPLES {
			let y = row as f32 + (sample as f32 + 0.5) * weight;

			crossings.clear();
			for edge in edges {
				let (top, bottom, winding) = if edge.from.y < edge.to.y {
					(edge.from, edge.to, 1)
				} else {
					(edge.to, edge.from, -1)
				};
				if y < top.y || y >= bottom.y {
					continue;
				}

				let x = top.x + (y - top.y) / (bottom.y - top.y) * (bottom.x - top.x);
				crossings.push((x, winding));
			}

			crossings.sort_by(|a, b| a.0.total_cmp(&b.0));

			let mut winding = 0;
			for pair in crossings.windows(2) {
				winding += pair[0].1;
				let inside = match fill_rule {
					FillRule::NonZero => winding != 0,
					FillRule::EvenOdd => winding % 2 != 0,
				};

				if inside {
					span(line, pair[0].0, pair[1].0, weight);
				}
			}
		}
	}
}

fn span(line: &mut [f32], from: f32, to: f32, weight: f32) {
	let from = from.max(0.0);
	let to = to.min(line.len() as f32);
	if from >= to {
		return;
	}

	let first = from as usize;
	let last = to as usize;
	if first == last {
		line[first] += (to - from) * weight;
		return;
	}

	line[first] += (first as f32 + 1.0 - from) * weight;
	for value in &mut line[first + 1..last] {
		*value += weight;
	}

	if last < line.len() {
		line[last] += (to - last as f32) * weight;
	}
}
