#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod edit;
mod icon;
mod menu;
mod mesh;
mod panel;
mod svg;
mod toolbar;
mod view;

use edit::EditMode;
use eframe::egui;
use mesh::Mesh;
use toolbar::{Tool, Toolbar};
use view::View;

const ICON_PNG: &[u8] = include_bytes!("../res/kricon.png");
const GRID_SPACING: f32 = 32.0;
const BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_rgb(24, 24, 27);
const GRID_COLOR: egui::Color32 = egui::Color32::from_rgb(44, 44, 48);
const DEFAULT_ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);
const MARGIN: f32 = 12.0;

struct App {
	toolbar: Toolbar,
	view: View,
	mesh: Mesh,
	edit: EditMode,
	accent: egui::Color32,
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
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let rect = ui.max_rect();
		let response = ui.interact(
			rect,
			egui::Id::new("viewport"),
			egui::Sense::click_and_drag(),
		);

		self.view.update(&response);

		if self.toolbar.active == Some(Tool::Edit) {
			self.edit.update(&mut self.mesh, &self.view, &response);
		} else {
			self.edit.cancel(&mut self.mesh);
		}

		let painter = ui.painter();
		painter.rect_filled(rect, 0.0, BACKGROUND_COLOR);
		self.draw_grid(painter, rect);
		self.edit.draw(&self.mesh, &self.view, painter, self.accent);

		self.toolbar.show(ui.ctx());
		self.edit.show_menu(ui.ctx(), &mut self.mesh, &self.view);
		self.show_accent_picker(ui.ctx());
	}
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
			Ok(Box::new(App {
				toolbar: Toolbar::new(),
				view: View {
					offset: egui::Vec2::ZERO,
					scale: GRID_SPACING,
				},
				mesh: Mesh::default(),
				edit: EditMode::new(),
				accent: DEFAULT_ACCENT_COLOR,
			}))
		}),
	)
}
