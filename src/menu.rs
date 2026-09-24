use crate::icon::Icon;
use crate::panel;
use eframe::egui;

const ITEM_WIDTH: f32 = 160.0;
const ITEM_HEIGHT: f32 = 28.0;
const ICON_SIZE: f32 = 16.0;
const PADDING: f32 = 8.0;
const TEXT_SIZE: f32 = 14.0;

pub struct Menu<T> {
	items: Vec<(T, &'static str, Icon)>,
}

impl<T: Copy> Menu<T> {
	pub fn new(items: &[(T, &'static str, &'static str)]) -> Self {
		Self {
			items: items
				.iter()
				.map(|&(action, label, icon)| (action, label, Icon::new(icon)))
				.collect(),
		}
	}

	pub fn show(&mut self, ctx: &egui::Context, pos: egui::Pos2) -> Option<T> {
		let icon_pixels = (ICON_SIZE * ctx.pixels_per_point()).round() as usize;
		let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
		let mut chosen = None;

		egui::Area::new(egui::Id::new("context_menu"))
			.order(egui::Order::Foreground)
			.fixed_pos(pos)
			.fade_in(false)
			.show(ctx, |ui| {
				panel::frame().show(ui, |ui| {
					ui.spacing_mut().item_spacing.y = 2.0;

					for (action, label, icon) in &mut self.items {
						let (rect, response, color) =
							panel::item(ui, egui::vec2(ITEM_WIDTH, ITEM_HEIGHT), false);
						if response.clicked() {
							chosen = Some(*action);
						}

						let icon_rect = egui::Rect::from_center_size(
							rect.left_center() + egui::vec2(PADDING + ICON_SIZE / 2.0, 0.0),
							egui::Vec2::splat(ICON_SIZE),
						);
						ui.painter()
							.image(icon.texture(ctx, icon_pixels), icon_rect, uv, color);
						ui.painter().text(
							icon_rect.right_center() + egui::vec2(PADDING, 0.0),
							egui::Align2::LEFT_CENTER,
							*label,
							egui::FontId::proportional(TEXT_SIZE),
							color,
						);
					}
				});
			});

		chosen
	}
}
