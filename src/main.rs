#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod edit;
mod icon;
mod layers;
mod menu;
mod mesh;
mod panel;
mod svg;
mod view;

use edit::EditMode;
use eframe::egui;
use layers::Layers;
use mesh::Mesh;
use view::View;

const ICON_PNG: &[u8] = include_bytes!("../res/kricon.png");
const GRID_SPACING: f32 = 32.0;
const BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_rgb(24, 24, 27);
const GRID_COLOR: egui::Color32 = egui::Color32::from_rgb(44, 44, 48);
const DEFAULT_ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);
const FACE_COLOR: egui::Color32 = egui::Color32::WHITE;
const DEFAULT_FACE_OPACITY: f32 = 0.1;
const MARGIN: f32 = 12.0;

struct App {
	view: View,
	mesh: Mesh,
	edit: EditMode,
	layers: Layers,
	accent: egui::Color32,
	face_opacity: f32,
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
		let color = FACE_COLOR.gamma_multiply(self.face_opacity);
		let mut shape = egui::Mesh::default();
		for &vertex in &self.mesh.vertices {
			shape.colored_vertex(self.view.to_screen(vertex), color);
		}

		for [a, b, c] in self.mesh.triangles() {
			shape.add_triangle(a as u32, b as u32, c as u32);
		}

		painter.add(shape);
	}

	fn show_accent_picker(&mut self, ctx: &egui::Context) {
		egui::Area::new(egui::Id::new("accent_picker"))
			.anchor(egui::Align2::LEFT_BOTTOM, [MARGIN, -MARGIN])
			.show(ctx, |ui| {
				egui::color_picker::color_edit_button_srgba(
					ui,
					&mut self.accent,
					egui::color_picker::Alpha::Opaque,
				);
			});
	}

	fn show_face_opacity(&mut self, ctx: &egui::Context) {
		egui::Area::new(egui::Id::new("face_opacity"))
			.anchor(egui::Align2::RIGHT_BOTTOM, [-MARGIN, -MARGIN])
			.show(ctx, |ui| {
				ui.horizontal(|ui| {
					opacity_slider(ui, &mut self.face_opacity);
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

		let painter = ui.painter();
		painter.rect_filled(rect, 0.0, BACKGROUND_COLOR);
		self.draw_grid(painter, rect);
		self.draw_faces(painter);
		self.edit.draw(&self.mesh, &self.view, painter, self.accent);

		self.edit
			.show_menu(ui.ctx(), &mut self.mesh, &self.view, self.accent);
		self.layers
			.show(ui.ctx(), &mut self.mesh, &mut self.edit, self.accent);
		self.show_accent_picker(ui.ctx());
		self.show_face_opacity(ui.ctx());
	}
}

fn opacity_slider(ui: &mut egui::Ui, value: &mut f32) {
	let size = egui::vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
	let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
	let radius = rect.height() / 2.5;
	let range = rect.x_range().shrink(radius);
	if let Some(pointer) = response.interact_pointer_pos() {
		*value = egui::remap_clamp(pointer.x, range.min..=range.max, 0.0..=1.0);
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

	let knob = egui::pos2(egui::lerp(range.min..=range.max, *value), rect.center().y);
	let expansion = ui.style().interact(&response).expansion;
	ui.painter()
		.circle_filled(knob, radius + expansion, DEFAULT_ACCENT_COLOR);
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
				accent: DEFAULT_ACCENT_COLOR,
				face_opacity: DEFAULT_FACE_OPACITY,
			}))
		}),
	)
}
