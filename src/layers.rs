use crate::edit::{self, EditMode};
use crate::icon::{self, Icon};
use crate::mesh::Mesh;
use crate::panel::{self, Header};
use eframe::egui;
use std::collections::HashMap;

const PANEL_WIDTH: f32 = 200.0;
const INDICATOR_SIZE: f32 = 8.0;
const INDICATOR_GAP: f32 = 2.0;
const DROP_COLOR: egui::Color32 = egui::Color32::WHITE;
const MARGIN: f32 = 12.0;
const ICON_GAP: f32 = 4.0;

pub struct Layers {
	header: Header,
	edit_icon: Icon,
	delete_icon: Icon,
	editing: Option<(u32, String)>,
}

impl Layers {
	pub fn new() -> Self {
		Self {
			header: Header::new("Layers", icon::BORING),
			edit_icon: Icon::new(icon::BORING),
			delete_icon: Icon::new(icon::BORING),
			editing: None,
		}
	}

	pub fn show(
		&mut self,
		ctx: &egui::Context,
		mesh: &mut Mesh,
		edit: &mut EditMode,
		accent: egui::Color32,
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
		let mut renamed = None;
		let mut deleted = None;

		egui::Window::new("Layers")
			.frame(panel::bordered_frame(accent))
			.title_bar(false)
			.pivot(egui::Align2::RIGHT_TOP)
			.default_pos(ctx.content_rect().right_top() + egui::vec2(-MARGIN, MARGIN))
			.resizable(false)
			.collapsible(false)
			.show(ctx, |ui| {
				ui.spacing_mut().item_spacing.y = 2.0;
				ui.set_width(PANEL_WIDTH);

				self.header.show(ui, PANEL_WIDTH, accent);

				if mesh.layers.is_empty() {
					let (rect, _) = ui.allocate_exact_size(
						egui::vec2(PANEL_WIDTH, panel::ITEM_HEIGHT + 2.0 * panel::PADDING),
						egui::Sense::hover(),
					);
					ui.painter().text(
						rect.center(),
						egui::Align2::CENTER_CENTER,
						"No layers",
						egui::FontId::proportional(panel::TEXT_SIZE),
						panel::CONTENT_COLOR,
					);
				}

				let mut rows = Vec::new();
				let mut dragged = None;
				for (index, layer) in mesh.layers.iter().enumerate() {
					let id = layer.id;
					let (rect, response, color) = panel::item(
						ui,
						egui::vec2(PANEL_WIDTH, panel::ITEM_HEIGHT),
						false,
						egui::Sense::click_and_drag(),
					);
					if response.clicked() {
						clicked = Some(id);
					}
					if response.dragged() || response.drag_stopped() {
						dragged = Some((index, response.drag_stopped()));
					}

					let indicator = egui::Rect::from_center_size(
						rect.left_center() + egui::vec2(panel::PADDING + INDICATOR_SIZE / 2.0, 0.0),
						egui::Vec2::splat(INDICATOR_SIZE),
					);
					match selected.get(&id) {
						Some(count) if *count == sizes[&id] => {
							ui.painter().rect_filled(indicator, 0.0, accent);
						}
						Some(_) => draw_partial(ui.painter(), indicator, accent),
						None => {}
					}

					let delete_rect = egui::Rect::from_center_size(
						rect.right_center()
							- egui::vec2(panel::PADDING + panel::ICON_SIZE / 2.0, 0.0),
						egui::Vec2::splat(panel::ICON_SIZE),
					);
					let edit_rect =
						delete_rect.translate(egui::vec2(-panel::ICON_SIZE - ICON_GAP, 0.0));
					if icon_button(ui, &mut self.delete_icon, delete_rect, color, "Delete", id)
						.clicked()
					{
						deleted = Some(id);
					}

					let name = if !layer.name.is_empty() {
						layer.name.clone()
					} else if layer.curve {
						format!("Curve {id}")
					} else {
						format!("Shape {id}")
					};
					if icon_button(ui, &mut self.edit_icon, edit_rect, color, "Rename", id)
						.clicked()
					{
						self.editing = Some((id, name.clone()));
					}

					match &mut self.editing {
						Some((editing, text)) if *editing == id => {
							let text_rect = egui::Rect::from_x_y_ranges(
								indicator.right() + panel::PADDING..=edit_rect.left() - ICON_GAP,
								rect.y_range(),
							);
							let response = ui.put(
								text_rect,
								egui::TextEdit::singleline(text)
									.font(egui::FontId::proportional(panel::TEXT_SIZE))
									.vertical_align(egui::Align::Center),
							);
							if !response.has_focus() && !response.lost_focus() {
								response.request_focus();
							} else if response.lost_focus() {
								if !ui.input(|input| input.key_pressed(egui::Key::Escape)) {
									renamed = Some((id, text.trim().to_string()));
								}
								self.editing = None;
							}
						}
						_ => {
							let label = if layer.holdout {
								format!("{name} (Holdout)")
							} else {
								name
							};
							panel::paint_label(ui, indicator, &label, color);
						}
					}
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
			edit.move_layer(mesh, from, target);
		}

		if let Some((layer, name)) = renamed {
			edit.rename_layer(mesh, layer, name);
		}

		if let Some(layer) = deleted {
			edit.delete_layer(mesh, layer);
		}

		if let Some(layer) = clicked {
			let shift = ctx.input(|input| input.modifiers.shift);
			edit.select_layer(mesh, layer, shift);
		}
	}
}

fn icon_button(
	ui: &mut egui::Ui,
	icon: &mut Icon,
	rect: egui::Rect,
	color: egui::Color32,
	hint: &str,
	layer: u32,
) -> egui::Response {
	let response = ui
		.interact(rect, ui.id().with((hint, layer)), egui::Sense::click())
		.on_hover_text(hint);
	let tint = if response.hovered() {
		panel::CONTENT_ACTIVE_COLOR
	} else {
		color
	};
	panel::paint_icon(ui, icon, rect, tint);
	response
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
