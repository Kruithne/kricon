use crate::view::View;
use eframe::egui::{
	self, Color32, ColorImage, DroppedFileHandle, Pos2, TextureHandle, TextureOptions,
	epaint::Vertex, pos2, vec2,
};
use image::imageops::FilterType;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const IMAGE_SIZE: f32 = 8.0;
const MAX_IMAGE_SIDE: usize = 4096;
const UVS: [Pos2; 4] = [
	pos2(0.0, 0.0),
	pos2(1.0, 0.0),
	pos2(1.0, 1.0),
	pos2(0.0, 1.0),
];

pub struct Decoded {
	pub source: Arc<[u8]>,
	pub pixels: ColorImage,
}

#[derive(Clone)]
pub struct Image {
	texture: TextureHandle,
	pub source: Arc<[u8]>,
	pub corners: [Pos2; 4],
}

impl PartialEq for Image {
	fn eq(&self, other: &Self) -> bool {
		self.texture == other.texture && self.corners == other.corners
	}
}

impl Image {
	pub fn new(ctx: &egui::Context, decoded: Decoded, center: Pos2) -> Self {
		let [width, height] = decoded.pixels.size.map(|side| side as f32);
		let half = vec2(width, height) * (IMAGE_SIZE / width.max(height) / 2.0);
		let corners = [
			center - half,
			center + vec2(half.x, -half.y),
			center + half,
			center + vec2(-half.x, half.y),
		];
		Self::with_corners(ctx, decoded, corners)
	}

	pub fn with_corners(ctx: &egui::Context, decoded: Decoded, corners: [Pos2; 4]) -> Self {
		Self {
			texture: ctx.load_texture("image", decoded.pixels, TextureOptions::LINEAR),
			source: decoded.source,
			corners,
		}
	}

	pub fn contains(&self, pos: Pos2) -> bool {
		crate::mesh::encloses(&self.corners, pos)
	}

	pub fn draw(&self, view: &View, painter: &egui::Painter) {
		let mut shape = egui::Mesh::with_texture(self.texture.id());
		for (&corner, uv) in self.corners.iter().zip(UVS) {
			shape.vertices.push(Vertex {
				pos: view.to_screen(corner),
				uv,
				color: Color32::WHITE,
			});
		}

		shape.add_triangle(0, 1, 2);
		shape.add_triangle(0, 2, 3);
		painter.add(shape);
	}
}

pub struct Loader {
	sender: Sender<Decoded>,
	receiver: Receiver<Decoded>,
}

impl Loader {
	pub fn new() -> Self {
		let (sender, receiver) = mpsc::channel();
		Self { sender, receiver }
	}

	pub fn load(&self, ctx: &egui::Context, files: Vec<DroppedFileHandle>) {
		let max_side = max_side(ctx);
		for file in files {
			let sender = self.sender.clone();
			let ctx = ctx.clone();
			thread::spawn(move || {
				if let Ok(bytes) = file.bytes()
					&& let Some(decoded) = decode(bytes.into(), max_side)
					&& sender.send(decoded).is_ok()
				{
					ctx.request_repaint();
				}
			});
		}
	}

	pub fn receive(&self) -> impl Iterator<Item = Decoded> {
		self.receiver.try_iter()
	}
}

pub fn max_side(ctx: &egui::Context) -> u32 {
	ctx.input(|input| input.max_texture_side)
		.min(MAX_IMAGE_SIDE) as u32
}

pub fn decode(source: Arc<[u8]>, max_side: u32) -> Option<Decoded> {
	let mut image = image::load_from_memory(&source).ok()?;
	if image.width().max(image.height()) > max_side {
		image = image.resize(max_side, max_side, FilterType::Triangle);
	}

	let rgba = image.to_rgba8();
	let size = [rgba.width() as usize, rgba.height() as usize];
	Some(Decoded {
		source,
		pixels: ColorImage::from_rgba_unmultiplied(size, &rgba),
	})
}
