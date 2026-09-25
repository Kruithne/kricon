#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod edit;
mod export;
mod geometry;
mod history;
mod icon;
mod images;
mod import;
mod layers;
mod menu;
mod mesh;
mod panel;
mod settings;
mod spacing;
mod svg;
mod view;
mod workspace;

use edit::{EditMode, FileAction};
use eframe::egui;
use icon::Icon;
use images::{Image, Loader};
use layers::Layers;
use mesh::Mesh;
use settings::Settings;
use spacing::Spacing;
use std::io;
use std::path::{Path, PathBuf};
use view::View;
use workspace::{Loaded, Meta, Workspace};

const ICON_PNG: &[u8] = include_bytes!("../res/kricon.png");
const GRID_SPACING: f32 = 32.0;
const BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_rgb(24, 24, 27);
const GRID_COLOR: egui::Color32 = egui::Color32::from_rgb(44, 44, 48);
const ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);
const MAX_SPACING: f32 = 8.0;
const SPACING_STEP: f32 = 0.5;
const MARGIN: f32 = 12.0;
const BUTTON_ICON_SIZE: f32 = 20.0;

struct App {
	view: View,
	mesh: Mesh,
	edit: EditMode,
	layers: Layers,
	face_opacity: f32,
	spacing: Spacing,
	loader: Loader,
	menu_icon: Icon,
	images_icon: Icon,
	outlines_icon: Icon,
	workspace: Workspace,
	settings: Settings,
	restoring: bool,
	title: String,
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
		let mut shape = egui::Mesh::default();
		for (triangle, color) in self.mesh.triangles() {
			let color = color.gamma_multiply(self.face_opacity);
			let index = shape.vertices.len() as u32;
			for pos in triangle {
				shape.colored_vertex(self.view.to_screen(pos), color);
			}
			shape.add_triangle(index, index + 1, index + 2);
		}

		painter.add(shape);
	}

	fn load_files(&mut self, ctx: &egui::Context, rect: egui::Rect) {
		let pos = self
			.view
			.to_world(ctx.pointer_latest_pos().unwrap_or(rect.center()));
		let (vectors, images): (Vec<_>, Vec<_>) = ctx
			.input(|input| input.raw.dropped_files.clone())
			.into_iter()
			.partition(|file| is_svg(file.path()));
		self.loader.load(ctx, images);

		for file in vectors {
			if let Ok(bytes) = file.bytes() {
				let shapes = import::parse(&String::from_utf8_lossy(&bytes));
				self.edit.import(&mut self.mesh, &shapes, pos);
			}
		}

		for decoded in self.loader.receive() {
			let image = Image::new(ctx, decoded, pos);
			self.edit.add_image(&mut self.mesh, image);
		}
	}

	fn handle_files(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
		if self.restoring
			&& !self.workspace.is_busy()
			&& let Some(path) = self.settings.workspace.clone()
		{
			self.workspace.load(ctx, path);
		}

		let request = self.edit.take_request();
		if !self.workspace.is_busy() {
			match request {
				Some(FileAction::New) => self.new_workspace(frame),
				Some(FileAction::Load) => self.load_workspace(ctx, frame),
				Some(FileAction::Save) => self.save_workspace(ctx, frame),
				None => {}
			}
		}

		if let Some(Err(error)) = self.workspace.poll_save() {
			show_error(frame, "Save Failed", &error);
		}

		match self.workspace.poll_load() {
			Some(Ok(loaded)) => self.open(ctx, loaded),
			Some(Err(error)) if !self.restoring => show_error(frame, "Load Failed", &error),
			_ => {}
		}

		if !self.workspace.is_busy() {
			self.restoring = false;
			self.sync_settings(ctx);
		}

		if self.workspace.autosave_due(ctx)
			&& self.is_dirty()
			&& !self.workspace.is_busy()
			&& let Some(path) = self.workspace.path.clone()
		{
			self.save(ctx, path);
		}

		self.update_title(ctx);
	}

	fn sync_settings(&mut self, ctx: &egui::Context) {
		if ctx.input(|input| input.pointer.any_down()) {
			return;
		}

		let settings = Settings {
			workspace: self.workspace.path.clone(),
			face_opacity: self.face_opacity,
			spacing: self.spacing.distance,
			show_images: self.edit.show_images,
			show_outlines: self.edit.show_outlines,
		};
		if settings != self.settings {
			self.settings = settings;
			self.settings.save();
		}
	}

	fn is_dirty(&self) -> bool {
		self.workspace.is_dirty(self.edit.revision())
	}

	fn confirm_discard(&self, frame: &eframe::Frame) -> bool {
		!self.is_dirty()
			|| rfd::MessageDialog::new()
				.set_parent(frame)
				.set_level(rfd::MessageLevel::Warning)
				.set_title("Unsaved Changes")
				.set_description("The workspace has unsaved changes. Discard them?")
				.set_buttons(rfd::MessageButtons::YesNo)
				.show() == rfd::MessageDialogResult::Yes
	}

	fn new_workspace(&mut self, frame: &eframe::Frame) {
		if !self.confirm_discard(frame) {
			return;
		}

		self.mesh = Mesh::default();
		self.edit.reset(&self.mesh);
		self.workspace.reset(None, self.edit.revision());
	}

	fn load_workspace(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
		if !self.confirm_discard(frame) {
			return;
		}

		if let Some(path) = file_dialog(frame).pick_file() {
			self.workspace.load(ctx, path);
		}
	}

	fn save_workspace(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
		let path = self
			.workspace
			.path
			.clone()
			.or_else(|| file_dialog(frame).save_file());
		if let Some(path) = path {
			self.save(ctx, path.with_extension(workspace::EXTENSION));
		}
	}

	fn save(&mut self, ctx: &egui::Context, path: PathBuf) {
		let meta = Meta {
			offset: self.view.offset,
			scale: self.view.scale,
		};
		self.workspace.save(
			ctx,
			path,
			self.edit.committed().clone(),
			meta,
			self.edit.revision(),
		);
	}

	fn open(&mut self, ctx: &egui::Context, loaded: Loaded) {
		let mut mesh = loaded.mesh;
		for (decoded, corners) in loaded.images {
			mesh.images.push(Image::with_corners(ctx, decoded, corners));
		}

		self.mesh = mesh;
		self.edit.reset(&self.mesh);
		self.workspace
			.reset(Some(loaded.path), self.edit.revision());
		if let Some(meta) = loaded.meta {
			self.view.offset = meta.offset;
			self.view.scale = meta.scale;
		}
	}

	fn update_title(&mut self, ctx: &egui::Context) {
		let name = self
			.workspace
			.path
			.as_deref()
			.and_then(Path::file_name)
			.map_or("Untitled".into(), |name| name.to_string_lossy());
		let dirty = if self.is_dirty() { "*" } else { "" };
		let title = format!("kricon - {name}{dirty}");
		if title != self.title {
			ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
			self.title = title;
		}
	}

	fn show_menu_button(&mut self, ctx: &egui::Context) {
		let open = self.edit.menu_open();
		let response = egui::Area::new(egui::Id::new("menu_button"))
			.anchor(egui::Align2::LEFT_TOP, [MARGIN, MARGIN])
			.show(ctx, |ui| button(ui, &mut self.menu_icon, open).clicked());

		if response.inner {
			let anchor =
				response.response.rect.left_bottom() + egui::vec2(0.0, panel::FRAME_MARGIN);
			self.edit.toggle_menu(&mut self.mesh, anchor);
		}
	}

	fn show_sliders(&mut self, ctx: &egui::Context) {
		egui::Area::new(egui::Id::new("sliders"))
			.anchor(egui::Align2::RIGHT_BOTTOM, [-MARGIN, -MARGIN])
			.show(ctx, |ui| {
				ui.horizontal(|ui| {
					let distance = &mut self.spacing.distance;
					if slider(ui, distance, MAX_SPACING)
						.on_hover_text("Minimum spacing")
						.interact_pointer_pos()
						.is_some()
					{
						*distance = (*distance / SPACING_STEP).round() * SPACING_STEP;
					}
					ui.add(
						egui::DragValue::new(distance)
							.range(0.0..=MAX_SPACING)
							.speed(MAX_SPACING / ui.spacing().slider_width),
					);

					slider(ui, &mut self.face_opacity, 1.0).on_hover_text("Face opacity");
					ui.add(
						egui::DragValue::new(&mut self.face_opacity)
							.range(0.0..=1.0)
							.speed(1.0 / ui.spacing().slider_width),
					);

					let images = &mut self.edit.show_images;
					if button(ui, &mut self.images_icon, *images)
						.on_hover_text("Toggle Images")
						.clicked()
					{
						*images = !*images;
					}

					let outlines = &mut self.edit.show_outlines;
					if button(ui, &mut self.outlines_icon, *outlines)
						.on_hover_text("Toggle Outlines")
						.clicked()
					{
						*outlines = !*outlines;
					}
				});
			});
	}
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
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
		self.handle_files(ui.ctx(), frame);
		self.load_files(ui.ctx(), rect);

		let painter = ui.painter();
		painter.rect_filled(rect, 0.0, BACKGROUND_COLOR);
		self.draw_grid(painter, rect);
		if self.edit.show_images {
			for image in &self.mesh.images {
				image.draw(&self.view, painter);
			}
		}
		self.draw_faces(painter);
		self.edit
			.draw(&self.mesh, &self.view, painter, ACCENT_COLOR);
		self.spacing.update(&self.mesh);
		self.spacing.draw(&self.view, painter);

		self.show_menu_button(ui.ctx());
		self.edit
			.show_menu(ui.ctx(), &mut self.mesh, &self.view, ACCENT_COLOR);
		self.edit
			.show_palette(ui.ctx(), &mut self.mesh, &self.view, ACCENT_COLOR);
		self.layers
			.show(ui.ctx(), &mut self.mesh, &mut self.edit, ACCENT_COLOR);
		self.show_sliders(ui.ctx());
	}
}

fn button(ui: &mut egui::Ui, icon: &mut Icon, active: bool) -> egui::Response {
	panel::frame()
		.show(ui, |ui| {
			let (rect, response, tint) = panel::item(
				ui,
				egui::Vec2::splat(panel::BUTTON_SIZE),
				active,
				egui::Sense::click(),
			);
			let icon_rect =
				egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(BUTTON_ICON_SIZE));
			panel::paint_icon(ui, icon, icon_rect, tint);
			response
		})
		.inner
}

fn is_svg(path: &Path) -> bool {
	path.extension()
		.is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

fn file_dialog(frame: &eframe::Frame) -> rfd::FileDialog {
	rfd::FileDialog::new()
		.set_parent(frame)
		.add_filter("kricon Workspace", &[workspace::EXTENSION])
}

fn show_error(frame: &eframe::Frame, title: &str, error: &io::Error) {
	rfd::MessageDialog::new()
		.set_parent(frame)
		.set_level(rfd::MessageLevel::Error)
		.set_title(title)
		.set_description(error.to_string())
		.show();
}

fn slider(ui: &mut egui::Ui, value: &mut f32, max: f32) -> egui::Response {
	let size = egui::vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
	let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
	let radius = rect.height() / 2.5;
	let range = rect.x_range().shrink(radius);
	if let Some(pointer) = response.interact_pointer_pos() {
		*value = egui::remap_clamp(pointer.x, range.min..=range.max, 0.0..=max);
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

	let knob = egui::pos2(
		egui::lerp(range.min..=range.max, *value / max),
		rect.center().y,
	);
	let expansion = ui.style().interact(&response).expansion;
	ui.painter()
		.circle_filled(knob, radius + expansion, ACCENT_COLOR);
	response
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
			let settings = Settings::load();
			let mut edit = EditMode::default();
			edit.show_images = settings.show_images;
			edit.show_outlines = settings.show_outlines;
			let mut spacing = Spacing::default();
			spacing.distance = settings.spacing.clamp(0.0, MAX_SPACING);
			Ok(Box::new(App {
				view: View {
					offset: egui::Vec2::ZERO,
					scale: GRID_SPACING,
				},
				mesh: Mesh::default(),
				edit,
				layers: Layers::default(),
				face_opacity: settings.face_opacity.clamp(0.0, 1.0),
				spacing,
				loader: Loader::default(),
				menu_icon: Icon::new(icon::MENU),
				images_icon: Icon::new(icon::BORING),
				outlines_icon: Icon::new(icon::BORING),
				workspace: Workspace::default(),
				restoring: settings.workspace.is_some(),
				settings,
				title: String::new(),
			}))
		}),
	)
}
