mod path;
mod raster;

pub use path::{Command, parse as parse_path};
pub use raster::rasterize;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
	pub x: f32,
	pub y: f32,
}

impl Point {
	pub fn new(x: f32, y: f32) -> Self {
		Self { x, y }
	}
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FillRule {
	NonZero,
	EvenOdd,
}

pub struct Path {
	pub commands: Vec<Command>,
	pub fill_rule: FillRule,
}

pub struct Svg {
	pub view_box: [f32; 4],
	pub paths: Vec<Path>,
}

impl Svg {
	pub fn parse(text: &str) -> Option<Self> {
		let mut view_box = None;
		let mut paths = Vec::new();

		for tag in tags(text) {
			let name = tag
				.split(|c: char| c.is_whitespace() || c == '/')
				.next()
				.unwrap_or("");
			match name {
				"svg" => view_box = attribute(tag, "viewBox").and_then(parse_view_box),
				"path" => {
					let Some(data) = attribute(tag, "d") else {
						continue;
					};

					let fill_rule = match attribute(tag, "fill-rule") {
						Some("evenodd") => FillRule::EvenOdd,
						_ => FillRule::NonZero,
					};

					paths.push(Path {
						commands: path::parse(data),
						fill_rule,
					});
				}
				_ => {}
			}
		}

		Some(Self {
			view_box: view_box?,
			paths,
		})
	}
}

pub fn tags(text: &str) -> impl Iterator<Item = &str> {
	let mut rest = text;
	std::iter::from_fn(move || {
		loop {
			let start = rest.find('<')?;
			rest = &rest[start..];

			if let Some(comment) = rest.strip_prefix("<!--") {
				rest = &comment[comment.find("-->")? + 3..];
				continue;
			}

			let end = rest.find('>')?;
			let tag = &rest[1..end];
			rest = &rest[end + 1..];
			return Some(tag);
		}
	})
}

pub fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
	let mut rest = tag;
	while let Some(eq) = rest.find('=') {
		let key = rest[..eq].split_whitespace().last().unwrap_or("");
		let after = rest[eq + 1..].trim_start();
		let quote = after.chars().next()?;
		if quote != '"' && quote != '\'' {
			return None;
		}

		let len = after[1..].find(quote)?;
		if key == name {
			return Some(&after[1..1 + len]);
		}

		rest = &after[len + 2..];
	}

	None
}

fn parse_view_box(value: &str) -> Option<[f32; 4]> {
	let mut numbers = value
		.split(|c: char| c.is_whitespace() || c == ',')
		.filter(|s| !s.is_empty())
		.map(|s| s.parse().ok());
	let view_box = [
		numbers.next()??,
		numbers.next()??,
		numbers.next()??,
		numbers.next()??,
	];
	if view_box[2] <= 0.0 || view_box[3] <= 0.0 {
		return None;
	}

	Some(view_box)
}
