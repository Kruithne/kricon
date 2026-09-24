use eframe::egui::{self, PointerButton, Pos2, Vec2};

const ZOOM_SPEED: f32 = 0.002;
const MIN_SCALE: f32 = 8.0;
const MAX_SCALE: f32 = 512.0;

pub struct View {
	pub offset: Vec2,
	pub scale: f32,
}

impl View {
	pub fn to_screen(&self, pos: Pos2) -> Pos2 {
		pos * self.scale + self.offset
	}

	pub fn to_world(&self, pos: Pos2) -> Pos2 {
		(pos - self.offset) / self.scale
	}

	pub fn update(&mut self, response: &egui::Response, zoom: bool, pan: bool) {
		if !response.hovered() {
			return;
		}

		let (scroll, cursor) = response
			.ctx
			.input(|input| (input.smooth_scroll_delta.y, input.pointer.latest_pos()));

		if pan && response.dragged_by(PointerButton::Middle) {
			self.offset += response.drag_delta();
		}

		if zoom
			&& let Some(cursor) = cursor
			&& scroll != 0.0
		{
			let anchor = self.to_world(cursor);
			self.scale = (self.scale * (scroll * ZOOM_SPEED).exp()).clamp(MIN_SCALE, MAX_SCALE);
			self.offset = cursor - anchor * self.scale;
		}
	}
}
