#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod edit;
mod history;
mod icon;
mod images;
mod layers;
mod menu;
mod mesh;
mod panel;
mod spacing;
mod svg;
mod view;

use edit::EditMode;
use eframe::egui;
use icon::Icon;
use images::{Image, Loader};
use layers::Layers;
use mesh::Mesh;
use spacing::Spacing;
use view::View;

const ICON_PNG: &[u8] = include_bytes!("../res/kricon.png");
const GRID_SPACING: f32 = 32.0;
const BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_rgb(24, 24, 27);
const GRID_COLOR: egui::Color32 = egui::Color32::from_rgb(44, 44, 48);
const ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);
const DEFAULT_FACE_OPACITY: f32 = 0.1;
const MAX_SPACING: f32 = 8.0;
const SPACING_STEP: f32 = 0.5;
const MARGIN: f32 = 12.0;
const BUTTON_ICON_SIZE: f32 = 20.0;

struct App {
	view: View,
	mesh: Mesh,
	edit: EditMode,
	layers: Layers,
	face_opacity: f32,
	spacing: Spacing,
	loader: Loader,
	menu_icon: Icon,
}

impl App {
	fn draw_grid(&self, painter: &egui::Painter, rect: egui::Rect) {
		let stroke = egui::Stroke::new(1.0, GRID_COLOR);
		let start = self.view.to_screen(self.view.to_world(rect.min).ceil());

		let mut x = start.x;
		while x <= rect.right() {
			painter.vline(x, rect.y_range(), stroke);
			x += self.view.scale;
		}

		let mut y = start.y;
		while y <= rect.bottom() {
			painter.hline(rect.x_range(), y, stroke);
			y += self.view.scale;
		}
	}

	fn draw_faces(&self, painter: &egui::Painter) {
		let mut shape = egui::Mesh::default();
		for (triangle, color) in self.mesh.triangles() {
			let color = color.gamma_multiply(self.face_opacity);
			let index = shape.vertices.len() as u32;
			for pos in triangle {
				shape.colored_vertex(self.view.to_screen(pos), color);
			}
			shape.add_triangle(index, index + 1, index + 2);
		}

		painter.add(shape);
	}

	fn load_images(&mut self, ctx: &egui::Context, rect: egui::Rect) {
		let dropped = ctx.input(|input| input.raw.dropped_files.clone());
		self.loader.load(ctx, dropped);

		for pixels in self.loader.receive() {
			let pos = ctx.pointer_latest_pos().unwrap_or(rect.center());
			let image = Image::new(ctx, pixels, self.view.to_world(pos));
			self.edit.add_image(&mut self.mesh, image);
		}
	}

	fn show_menu_button(&mut self, ctx: &egui::Context) {
		let open = self.edit.menu_open();
		let response = egui::Area::new(egui::Id::new("menu_button"))
			.anchor(egui::Align2::LEFT_TOP, [MARGIN, MARGIN])
			.show(ctx, |ui| {
				panel::frame()
					.show(ui, |ui| {
						let (rect, response, tint) = panel::item(
							ui,
							egui::Vec2::splat(panel::BUTTON_SIZE),
							open,
							egui::Sense::click(),
						);
						let icon_rect = egui::Rect::from_center_size(
							rect.center(),
							egui::Vec2::splat(BUTTON_ICON_SIZE),
						);
						panel::paint_icon(ui, &mut self.menu_icon, icon_rect, tint);
						response.clicked()
					})
					.inner
			});

		if response.inner {
			let anchor =
				response.response.rect.left_bottom() + egui::vec2(0.0, panel::FRAME_MARGIN);
			self.edit.toggle_menu(&mut self.mesh, anchor);
		}
	}

	fn show_sliders(&mut self, ctx: &egui::Context) {
		egui::Area::new(egui::Id::new("sliders"))
			.anchor(egui::Align2::RIGHT_BOTTOM, [-MARGIN, -MARGIN])
			.show(ctx, |ui| {
				ui.horizontal(|ui| {
					let distance = &mut self.spacing.distance;
					if slider(ui, distance, MAX_SPACING)
						.on_hover_text("Minimum spacing")
						.interact_pointer_pos()
						.is_some()
					{
						*distance = (*distance / SPACING_STEP).round() * SPACING_STEP;
					}
					ui.add(
						egui::DragValue::new(distance)
							.range(0.0..=MAX_SPACING)
							.speed(MAX_SPACING / ui.spacing().slider_width),
					);

					slider(ui, &mut self.face_opacity, 1.0).on_hover_text("Face opacity");
					ui.add(
						egui::DragValue::new(&mut self.face_opacity)
							.range(0.0..=1.0)
							.speed(1.0 / ui.spacing().slider_width),
					);
				});
			});
	}
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let rect = ui.max_rect();
		let response = ui.interact(
			rect,
			egui::Id::new("viewport"),
			egui::Sense::click_and_drag(),
		);

		self.view.update(
			&response,
			!self.edit.captures_scroll(),
			!self.edit.captures_middle(),
		);

		self.edit.update(&mut self.mesh, &self.view, &response);
		self.load_images(ui.ctx(), rect);

		let painter = ui.painter();
		painter.rect_filled(rect, 0.0, BACKGROUND_COLOR);
		self.draw_grid(painter, rect);
		for image in &self.mesh.images {
			image.draw(&self.view, painter);
		}
		self.draw_faces(painter);
		self.edit
			.draw(&self.mesh, &self.view, painter, ACCENT_COLOR);
		self.spacing.update(&self.mesh);
		self.spacing.draw(&self.view, painter);

		self.show_menu_button(ui.ctx());
		self.edit
			.show_menu(ui.ctx(), &mut self.mesh, &self.view, ACCENT_COLOR);
		self.edit
			.show_palette(ui.ctx(), &mut self.mesh, &self.view, ACCENT_COLOR);
		self.layers
			.show(ui.ctx(), &mut self.mesh, &mut self.edit, ACCENT_COLOR);
		self.show_sliders(ui.ctx());
	}
}

fn slider(ui: &mut egui::Ui, value: &mut f32, max: f32) -> egui::Response {
	let size = egui::vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
	let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
	let radius = rect.height() / 2.5;
	let range = rect.x_range().shrink(radius);
	if let Some(pointer) = response.interact_pointer_pos() {
		*value = egui::remap_clamp(pointer.x, range.min..=range.max, 0.0..=max);
	}

	let rail = egui::Rect::from_center_size(
		rect.center(),
		egui::vec2(rect.width(), ui.spacing().slider_rail_height),
	);
	let widgets = &ui.visuals().widgets;
	ui.painter().rect_filled(
		rail,
		widgets.inactive.corner_radius,
		widgets.inactive.bg_fill,
	);

	let knob = egui::pos2(
		egui::lerp(range.min..=range.max, *value / max),
		rect.center().y,
	);
	let expansion = ui.style().interact(&response).expansion;
	ui.painter()
		.circle_filled(knob, radius + expansion, ACCENT_COLOR);
	response
}

fn main() -> eframe::Result {
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default()
			.with_title("kricon")
			.with_icon(eframe::icon_data::from_png_bytes(ICON_PNG).unwrap())
			.with_maximized(true),
		..Default::default()
	};

	eframe::run_native(
		"kricon",
		options,
		Box::new(|cc| {
			cc.egui_ctx.set_theme(egui::Theme::Dark);
			cc.egui_ctx
				.all_styles_mut(|style| style.animation_time = 0.0);
			Ok(Box::new(App {
				view: View {
					offset: egui::Vec2::ZERO,
					scale: GRID_SPACING,
				},
				mesh: Mesh::default(),
				edit: EditMode::new(),
				layers: Layers::new(),
				face_opacity: DEFAULT_FACE_OPACITY,
				spacing: Spacing::default(),
				loader: Loader::new(),
				menu_icon: Icon::new(icon::MENU),
			}))
		}),
	)
}
