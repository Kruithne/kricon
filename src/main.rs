mod icon;
mod svg;
mod toolbar;

use eframe::egui;
use toolbar::Toolbar;

const ICON_PNG: &[u8] = include_bytes!("../res/kricon.png");
const GRID_SPACING: f32 = 32.0;
const BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_rgb(24, 24, 27);
const GRID_COLOR: egui::Color32 = egui::Color32::from_rgb(44, 44, 48);

struct App {
	toolbar: Toolbar,
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let rect = ui.max_rect();
		let painter = ui.painter();
		let stroke = egui::Stroke::new(1.0, GRID_COLOR);

		painter.rect_filled(rect, 0.0, BACKGROUND_COLOR);

		let mut x = (rect.left() / GRID_SPACING).ceil() * GRID_SPACING;
		while x <= rect.right() {
			painter.vline(x, rect.y_range(), stroke);
			x += GRID_SPACING;
		}

		let mut y = (rect.top() / GRID_SPACING).ceil() * GRID_SPACING;
		while y <= rect.bottom() {
			painter.hline(rect.x_range(), y, stroke);
			y += GRID_SPACING;
		}

		self.toolbar.show(ui.ctx());
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
			}))
		}),
	)
}
