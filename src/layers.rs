use crate::edit::{self, EditMode};
use crate::icon::{self, Icon};
use crate::mesh::Mesh;
use crate::panel;
use crate::toolbar::Tool;
use eframe::egui;
use std::collections::HashMap;

const PANEL_WIDTH: f32 = 160.0;
const ITEM_HEIGHT: f32 = 28.0;
const ICON_SIZE: f32 = 16.0;
const CLOSE_SIZE: f32 = 20.0;
const CLOSE_ICON_SIZE: f32 = 10.0;
const INDICATOR_SIZE: f32 = 8.0;
const INDICATOR_GAP: f32 = 2.0;
const PADDING: f32 = 8.0;
const TEXT_SIZE: f32 = 14.0;
const DROP_COLOR: egui::Color32 = egui::Color32::WHITE;
const MARGIN: f32 = 12.0;
const BORDER_WIDTH: f32 = 1.0;

pub struct Layers {
	icon: Icon,
	close_icon: Icon,
}

impl Layers {
	pub fn new() -> Self {
		Self {
			icon: Icon::new(Tool::Layers.icon()),
			close_icon: Icon::new(icon::CLOSE),
		}
	}

	pub fn show(
		&mut self,
		ctx: &egui::Context,
		mesh: &mut Mesh,
		edit: &mut EditMode,
		accent: egui::Color32,
		open: &mut bool,
	) {
		let mut sizes: HashMap<u32, usize> = HashMap::new();
		for vertex in 0..mesh.vertices.len() {
			*sizes.entry(mesh.layer(vertex)).or_default() += 1;
		}

		let mut selected: HashMap<u32, usize> = HashMap::new();
		for &vertex in edit.selection() {
			*selected.entry(mesh.layer(vertex)).or_default() += 1;
		}
		let mut clicked = None;
		let mut moved = None;

		let icon_pixels = (ICON_SIZE * ctx.pixels_per_point()).round() as usize;
		let close_pixels = (CLOSE_ICON_SIZE * ctx.pixels_per_point()).round() as usize;
		let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

		egui::Window::new("Layers")
			.frame(panel::frame().stroke(egui::Stroke::new(BORDER_WIDTH, accent)))
			.title_bar(false)
			.pivot(egui::Align2::RIGHT_TOP)
			.default_pos(ctx.content_rect().right_top() + egui::vec2(-MARGIN, MARGIN))
			.resizable(false)
			.collapsible(false)
			.show(ctx, |ui| {
				ui.spacing_mut().item_spacing.y = 2.0;
				ui.set_width(PANEL_WIDTH);

				let (header, _) = ui.allocate_exact_size(
					egui::vec2(PANEL_WIDTH, ITEM_HEIGHT),
					egui::Sense::hover(),
				);
				let icon_rect = egui::Rect::from_center_size(
					header.left_center() + egui::vec2(PADDING + ICON_SIZE / 2.0, 0.0),
					egui::Vec2::splat(ICON_SIZE),
				);
				ui.painter().image(
					self.icon.texture(ctx, icon_pixels),
					icon_rect,
					uv,
					panel::CONTENT_ACTIVE_COLOR,
				);
				ui.painter().text(
					icon_rect.right_center() + egui::vec2(PADDING, 0.0),
					egui::Align2::LEFT_CENTER,
					"Layers",
					egui::FontId::proportional(TEXT_SIZE),
					panel::CONTENT_ACTIVE_COLOR,
				);

				let close_rect = egui::Rect::from_center_size(
					header.right_center() - egui::vec2(ITEM_HEIGHT / 2.0, 0.0),
					egui::Vec2::splat(CLOSE_SIZE),
				);
				let close = ui.interact(close_rect, ui.id().with("close"), egui::Sense::click());
				if close.clicked() {
					*open = false;
				}
				let close_color = panel::highlight(ui, close_rect, &close, false);
				ui.painter().image(
					self.close_icon.texture(ctx, close_pixels),
					egui::Rect::from_center_size(
						close_rect.center(),
						egui::Vec2::splat(CLOSE_ICON_SIZE),
					),
					uv,
					close_color,
				);

				ui.painter().hline(
					header.x_range().expand(panel::FRAME_MARGIN),
					header.bottom(),
					egui::Stroke::new(BORDER_WIDTH, accent),
				);

				let mut rows = Vec::new();
				let mut dragged = None;
				for (index, &layer) in mesh.layers.iter().enumerate() {
					let (rect, response, color) = panel::item(
						ui,
						egui::vec2(PANEL_WIDTH, ITEM_HEIGHT),
						false,
						egui::Sense::click_and_drag(),
					);
					if response.clicked() {
						clicked = Some(layer);
					}
					if response.dragged() || response.drag_stopped() {
						dragged = Some((index, response.drag_stopped()));
					}

					let indicator = egui::Rect::from_center_size(
						rect.left_center() + egui::vec2(PADDING + INDICATOR_SIZE / 2.0, 0.0),
						egui::Vec2::splat(INDICATOR_SIZE),
					);
					match selected.get(&layer) {
						Some(count) if *count == sizes[&layer] => {
							ui.painter().rect_filled(indicator, 0.0, accent);
						}
						Some(_) => draw_partial(ui.painter(), indicator, accent),
						None => {}
					}
					ui.painter().text(
						indicator.right_center() + egui::vec2(PADDING, 0.0),
						egui::Align2::LEFT_CENTER,
						format!("Shape {layer}"),
						egui::FontId::proportional(TEXT_SIZE),
						color,
					);
					rows.push(rect);
				}

				let Some((from, stopped)) = dragged else {
					return;
				};
				let Some(pointer) = ctx.pointer_latest_pos() else {
					return;
				};

				let target = rows.iter().filter(|row| row.center().y < pointer.y).count();
				if stopped {
					moved = Some((from, target));
					return;
				}

				let y = match target {
					0 => rows[0].top(),
					_ => rows[target - 1].bottom(),
				};
				ui.painter()
					.hline(rows[0].x_range(), y, egui::Stroke::new(2.0, DROP_COLOR));
			});

		if let Some((from, target)) = moved {
			let layer = mesh.layers.remove(from);
			let to = if target > from { target - 1 } else { target };
			mesh.layers.insert(to, layer);
		}

		if let Some(layer) = clicked {
			let shift = ctx.input(|input| input.modifiers.shift);
			edit.select_layer(mesh, layer, shift);
		}
	}
}

fn draw_partial(painter: &egui::Painter, rect: egui::Rect, accent: egui::Color32) {
	let size = egui::Vec2::splat((INDICATOR_SIZE - INDICATOR_GAP) / 2.0);
	let step = size.x + INDICATOR_GAP;
	for (x, y) in [(0.0, 0.0), (step, 0.0), (0.0, step), (step, step)] {
		let color = if x == 0.0 && y == 0.0 {
			edit::SELECTED_COLOR
		} else {
			accent
		};
		let square = egui::Rect::from_min_size(rect.min + egui::vec2(x, y), size);
		painter.rect_filled(square, 0.0, color);
	}
}
