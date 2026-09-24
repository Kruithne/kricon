use crate::icon::Icon;
use crate::panel::{self, Header};
use eframe::egui;

const ITEM_WIDTH: f32 = 160.0;

pub struct Menu<T> {
	header: Header,
	items: Vec<(T, &'static str, Icon)>,
}

impl<T: Copy> Menu<T> {
	pub fn new(title: &'static str, icon: &str, items: &[(T, &'static str, &'static str)]) -> Self {
		Self {
			header: Header::new(title, icon),
			items: items
				.iter()
				.map(|&(action, label, icon)| (action, label, Icon::new(icon)))
				.collect(),
		}
	}

	pub fn show(
		&mut self,
		ctx: &egui::Context,
		pos: egui::Pos2,
		accent: egui::Color32,
	) -> Option<T> {
		let mut chosen = None;

		egui::Area::new(egui::Id::new("context_menu"))
			.order(egui::Order::Foreground)
			.fixed_pos(pos)
			.show(ctx, |ui| {
				panel::bordered_frame(accent).show(ui, |ui| {
					ui.spacing_mut().item_spacing.y = 2.0;
					self.header.show(ui, ITEM_WIDTH, accent);

					for (action, label, icon) in &mut self.items {
						let (rect, response, color) = panel::item(
							ui,
							egui::vec2(ITEM_WIDTH, panel::ITEM_HEIGHT),
							false,
							egui::Sense::click(),
						);
						if response.clicked() {
							chosen = Some(*action);
						}

						let icon_rect = panel::icon_rect(rect);
						panel::paint_icon(ui, icon, icon_rect, color);
						panel::paint_label(ui, icon_rect, label, color);
					}
				});
			});

		chosen
	}
}
