use crate::view::View;
use eframe::egui::{
	self, Color32, ColorImage, DroppedFileHandle, Pos2, TextureHandle, TextureOptions,
	epaint::Vertex, pos2, vec2,
};
use image::imageops::FilterType;
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

#[derive(Clone, PartialEq)]
pub struct Image {
	texture: TextureHandle,
	pub corners: [Pos2; 4],
}

impl Image {
	pub fn new(ctx: &egui::Context, pixels: ColorImage, center: Pos2) -> Self {
		let [width, height] = pixels.size.map(|side| side as f32);
		let half = vec2(width, height) * (IMAGE_SIZE / width.max(height) / 2.0);
		Self {
			texture: ctx.load_texture("image", pixels, TextureOptions::LINEAR),
			corners: [
				center - half,
				center + vec2(half.x, -half.y),
				center + half,
				center + vec2(-half.x, half.y),
			],
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
	sender: Sender<ColorImage>,
	receiver: Receiver<ColorImage>,
}

impl Loader {
	pub fn new() -> Self {
		let (sender, receiver) = mpsc::channel();
		Self { sender, receiver }
	}

	pub fn load(&self, ctx: &egui::Context, files: Vec<DroppedFileHandle>) {
		let max_side = ctx
			.input(|input| input.max_texture_side)
			.min(MAX_IMAGE_SIDE) as u32;

		for file in files {
			let sender = self.sender.clone();
			let ctx = ctx.clone();
			thread::spawn(move || {
				if let Some(pixels) = decode(&file, max_side)
					&& sender.send(pixels).is_ok()
				{
					ctx.request_repaint();
				}
			});
		}
	}

	pub fn receive(&self) -> impl Iterator<Item = ColorImage> {
		self.receiver.try_iter()
	}
}

fn decode(file: &DroppedFileHandle, max_side: u32) -> Option<ColorImage> {
	let bytes = file.bytes().ok()?;
	let mut image = image::load_from_memory(&bytes).ok()?;
	if image.width().max(image.height()) > max_side {
		image = image.resize(max_side, max_side, FilterType::Triangle);
	}

	let rgba = image.to_rgba8();
	let size = [rgba.width() as usize, rgba.height() as usize];
	Some(ColorImage::from_rgba_unmultiplied(size, &rgba))
}
