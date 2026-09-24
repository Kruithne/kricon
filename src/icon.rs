use crate::svg::{self, Svg};
use eframe::egui;

pub const BORING: &str = include_str!("../res/icons/ico_boring.svg");
pub const CLOSE: &str = include_str!("../res/icons/ico_close.svg");

pub struct Icon {
	svg: Svg,
	texture: Option<(usize, egui::TextureHandle)>,
}

impl Icon {
	pub fn new(source: &str) -> Self {
		Self {
			svg: Svg::parse(source).expect("invalid icon svg"),
			texture: None,
		}
	}

	pub fn texture(&mut self, ctx: &egui::Context, size: usize) -> egui::TextureId {
		if let Some((cached, texture)) = &self.texture
			&& *cached == size
		{
			return texture.id();
		}

		let pixels = svg::rasterize(&self.svg, size, size)
			.into_iter()
			.map(egui::Color32::from_white_alpha)
			.collect();
		let image = egui::ColorImage::new([size, size], pixels);
		let texture = ctx.load_texture("icon", image, egui::TextureOptions::LINEAR);
		let id = texture.id();
		self.texture = Some((size, texture));
		id
	}
}
