use crate::icon::{self, Icon};
use eframe::egui;

const TOOL_COUNT: usize = 10;
const BUTTON_SIZE: f32 = 36.0;
const ICON_SIZE: f32 = 20.0;
const MARGIN: f32 = 12.0;
const PANEL_COLOR: egui::Color32 = egui::Color32::from_rgb(32, 32, 36);
const HOVER_COLOR: egui::Color32 = egui::Color32::from_rgb(48, 48, 54);
const SELECTED_COLOR: egui::Color32 = egui::Color32::from_rgb(60, 60, 68);
const ICON_COLOR: egui::Color32 = egui::Color32::from_rgb(160, 160, 168);
const ICON_ACTIVE_COLOR: egui::Color32 = egui::Color32::WHITE;

pub struct Toolbar {
	icon: Icon,
	selected: usize,
}

impl Toolbar {
	pub fn new() -> Self {
		Self {
			icon: Icon::new(icon::BORING),
			selected: 0,
		}
	}

	pub fn show(&mut self, ctx: &egui::Context) {
		let icon_pixels = (ICON_SIZE * ctx.pixels_per_point()).round() as usize;
		let texture = self.icon.texture(ctx, icon_pixels);
		let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

		egui::Area::new(egui::Id::new("toolbar"))
			.anchor(egui::Align2::LEFT_CENTER, [MARGIN, 0.0])
			.show(ctx, |ui| {
				egui::Frame::new()
					.fill(PANEL_COLOR)
					.corner_radius(8.0)
					.inner_margin(4.0)
					.show(ui, |ui| {
						ui.spacing_mut().item_spacing.y = 2.0;

						for tool in 0..TOOL_COUNT {
							let (rect, response) = ui.allocate_exact_size(
								egui::Vec2::splat(BUTTON_SIZE),
								egui::Sense::click(),
							);
							if response.clicked() {
								self.selected = tool;
							}

							let selected = tool == self.selected;
							let hovered = response.hovered();
							let painter = ui.painter();

							if selected {
								painter.rect_filled(rect, 6.0, SELECTED_COLOR);
							} else if hovered {
								painter.rect_filled(rect, 6.0, HOVER_COLOR);
							}

							let tint = if selected || hovered {
								ICON_ACTIVE_COLOR
							} else {
								ICON_COLOR
							};
							let icon_rect = egui::Rect::from_center_size(
								rect.center(),
								egui::Vec2::splat(ICON_SIZE),
							);
							painter.image(texture, icon_rect, uv, tint);
						}
					});
			});
	}
}
