use eframe::egui;

const PANEL_COLOR: egui::Color32 = egui::Color32::from_rgb(32, 32, 36);
const HOVER_COLOR: egui::Color32 = egui::Color32::from_rgb(48, 48, 54);
const SELECTED_COLOR: egui::Color32 = egui::Color32::from_rgb(60, 60, 68);
pub const FRAME_MARGIN: f32 = 4.0;
const CONTENT_COLOR: egui::Color32 = egui::Color32::from_rgb(160, 160, 168);
pub const CONTENT_ACTIVE_COLOR: egui::Color32 = egui::Color32::WHITE;

pub fn frame() -> egui::Frame {
	egui::Frame::new()
		.fill(PANEL_COLOR)
		.corner_radius(8.0)
		.inner_margin(FRAME_MARGIN)
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
