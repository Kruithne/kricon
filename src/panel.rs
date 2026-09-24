use crate::icon::Icon;
use eframe::egui;

const PANEL_COLOR: egui::Color32 = egui::Color32::from_rgb(32, 32, 36);
const HOVER_COLOR: egui::Color32 = egui::Color32::from_rgb(48, 48, 54);
const SELECTED_COLOR: egui::Color32 = egui::Color32::from_rgb(60, 60, 68);
pub const FRAME_MARGIN: f32 = 4.0;
pub const CONTENT_COLOR: egui::Color32 = egui::Color32::from_rgb(160, 160, 168);
pub const CONTENT_ACTIVE_COLOR: egui::Color32 = egui::Color32::WHITE;
pub const ITEM_HEIGHT: f32 = 28.0;
const ICON_SIZE: f32 = 16.0;
pub const PADDING: f32 = 8.0;
pub const TEXT_SIZE: f32 = 14.0;
const BORDER_WIDTH: f32 = 1.0;

pub fn frame() -> egui::Frame {
	egui::Frame::new()
		.fill(PANEL_COLOR)
		.corner_radius(8.0)
		.inner_margin(FRAME_MARGIN)
}

pub fn bordered_frame(accent: egui::Color32) -> egui::Frame {
	frame().stroke(egui::Stroke::new(BORDER_WIDTH, accent))
}

pub struct Header {
	title: &'static str,
	icon: Icon,
}

impl Header {
	pub fn new(title: &'static str, icon: &str) -> Self {
		Self {
			title,
			icon: Icon::new(icon),
		}
	}

	pub fn show(&mut self, ui: &mut egui::Ui, width: f32, accent: egui::Color32) {
		let (header, _) =
			ui.allocate_exact_size(egui::vec2(width, ITEM_HEIGHT), egui::Sense::hover());
		let icon_rect = icon_rect(header);
		paint_icon(ui, &mut self.icon, icon_rect, CONTENT_ACTIVE_COLOR);
		paint_label(ui, icon_rect, self.title, CONTENT_ACTIVE_COLOR);

		ui.painter().hline(
			header.x_range().expand(FRAME_MARGIN),
			header.bottom(),
			egui::Stroke::new(BORDER_WIDTH, accent),
		);
	}
}

pub fn icon_rect(row: egui::Rect) -> egui::Rect {
	egui::Rect::from_center_size(
		row.left_center() + egui::vec2(PADDING + ICON_SIZE / 2.0, 0.0),
		egui::Vec2::splat(ICON_SIZE),
	)
}

pub fn paint_icon(ui: &egui::Ui, icon: &mut Icon, rect: egui::Rect, tint: egui::Color32) {
	let pixels = (rect.width() * ui.ctx().pixels_per_point()).round() as usize;
	let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
	ui.painter()
		.image(icon.texture(ui.ctx(), pixels), rect, uv, tint);
}

pub fn paint_label(ui: &egui::Ui, after: egui::Rect, text: &str, color: egui::Color32) {
	ui.painter().text(
		after.right_center() + egui::vec2(PADDING, 0.0),
		egui::Align2::LEFT_CENTER,
		text,
		egui::FontId::proportional(TEXT_SIZE),
		color,
	);
}

pub fn item(
	ui: &mut egui::Ui,
	size: egui::Vec2,
	selected: bool,
	sense: egui::Sense,
) -> (egui::Rect, egui::Response, egui::Color32) {
	let (rect, response) = ui.allocate_exact_size(size, sense);
	let color = highlight(ui, rect, &response, selected);
	(rect, response, color)
}

pub fn highlight(
	ui: &egui::Ui,
	rect: egui::Rect,
	response: &egui::Response,
	selected: bool,
) -> egui::Color32 {
	let hovered = response.hovered();

	if selected {
		ui.painter().rect_filled(rect, 6.0, SELECTED_COLOR);
	} else if hovered {
		ui.painter().rect_filled(rect, 6.0, HOVER_COLOR);
	}

	if selected || hovered {
		CONTENT_ACTIVE_COLOR
	} else {
		CONTENT_COLOR
	}
}
