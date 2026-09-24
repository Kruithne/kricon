use crate::icon::{self, Icon};
use crate::panel;
use eframe::egui;

const TOOLS: [Tool; 3] = [Tool::Edit, Tool::Brush, Tool::Layers];
const BUTTON_SIZE: f32 = 36.0;
const ICON_SIZE: f32 = 20.0;
const MARGIN: f32 = 12.0;

#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
	Edit,
	Brush,
	Layers,
}

impl Tool {
	pub fn icon(self) -> &'static str {
		match self {
			Tool::Edit | Tool::Brush | Tool::Layers => icon::BORING,
		}
	}
}

pub struct Toolbar {
	icons: Vec<Icon>,
	pub active: Option<Tool>,
	pub layers: bool,
}

impl Toolbar {
	pub fn new() -> Self {
		Self {
			icons: TOOLS.iter().map(|tool| Icon::new(tool.icon())).collect(),
			active: None,
			layers: false,
		}
	}

	pub fn show(&mut self, ctx: &egui::Context, mode: Option<Tool>) -> Option<Tool> {
		let mut toggled = None;
		let icon_pixels = (ICON_SIZE * ctx.pixels_per_point()).round() as usize;
		let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

		egui::Area::new(egui::Id::new("toolbar"))
			.anchor(egui::Align2::LEFT_CENTER, [MARGIN, 0.0])
			.show(ctx, |ui| {
				panel::frame().show(ui, |ui| {
					ui.spacing_mut().item_spacing.y = 2.0;

					for (tool, icon) in TOOLS.into_iter().zip(&mut self.icons) {
						let selected = match tool {
							Tool::Layers => self.layers,
							Tool::Brush => mode == Some(tool),
							Tool::Edit => self.active == Some(tool),
						};
						let (rect, response, tint) = panel::item(
							ui,
							egui::Vec2::splat(BUTTON_SIZE),
							selected,
							egui::Sense::click(),
						);
						if response.clicked() {
							match tool {
								Tool::Layers => self.layers = !selected,
								Tool::Brush => {
									self.active = Some(Tool::Edit);
									toggled = Some(tool);
								}
								Tool::Edit => self.active = (!selected).then_some(tool),
							}
						}

						let icon_rect = egui::Rect::from_center_size(
							rect.center(),
							egui::Vec2::splat(ICON_SIZE),
						);
						ui.painter()
							.image(icon.texture(ctx, icon_pixels), icon_rect, uv, tint);
					}
				});
			});

		toggled
	}
}
