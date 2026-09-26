use crate::edit::{self, EditMode};
use crate::icon::{self, Icon};
use crate::mesh::Mesh;
use crate::panel::{self, Header};
use eframe::egui;
use std::collections::{HashMap, HashSet};

const PANEL_WIDTH: f32 = 240.0;
const INDICATOR_SIZE: f32 = 8.0;
const INDICATOR_GAP: f32 = 2.0;
const DROP_COLOR: egui::Color32 = egui::Color32::WHITE;
const MARGIN: f32 = 12.0;
const ICON_GAP: f32 = 4.0;
const INDENT: f32 = 12.0;
const ARROW_SIZE: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Hash, Debug)]
enum Item {
	Layer(u32),
	Group(u32),
}

struct Row {
	item: Item,
	start: usize,
	group: u32,
	name: String,
	label: String,
	selected: usize,
	total: usize,
	open: Option<bool>,
	mirror: bool,
}

pub struct Layers {
	header: Header,
	edit_icon: Icon,
	delete_icon: Icon,
	mirror_icon: Icon,
	editing: Option<(Item, String)>,
	expanded: HashSet<u32>,
}

impl Default for Layers {
	fn default() -> Self {
		Self {
			header: Header::new("Layers", Some(icon::LAYERS)),
			edit_icon: Icon::new(icon::EDIT),
			delete_icon: Icon::new(icon::TRASH),
			mirror_icon: Icon::new(icon::BORING),
			editing: None,
			expanded: HashSet::new(),
		}
	}
}

impl Layers {
	pub fn show(
		&mut self,
		ctx: &egui::Context,
		mesh: &mut Mesh,
		edit: &mut EditMode,
		accent: egui::Color32,
	) {
		let rows = self.rows(mesh, edit);
		let mut clicked = None;
		let mut moved = None;
		let mut renamed = None;
		let mut deleted = None;
		let mut toggled = None;
		let mut mirrored = None;

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

				if rows.is_empty() {
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

				let mut rects = Vec::new();
				let mut dragged = None;
				for (index, row) in rows.iter().enumerate() {
					let item = row.item;
					let (rect, response, color) = panel::item(
						ui,
						egui::vec2(PANEL_WIDTH, panel::ITEM_HEIGHT),
						false,
						egui::Sense::click_and_drag(),
					);
					if response.clicked() {
						clicked = Some(item);
					}
					if response.dragged() || response.drag_stopped() {
						dragged = Some((index, response.drag_stopped()));
					}

					let indent = if row.open.is_none() && row.group != 0 {
						INDENT
					} else {
						0.0
					};
					let indicator = egui::Rect::from_center_size(
						rect.left_center()
							+ egui::vec2(indent + panel::PADDING + INDICATOR_SIZE / 2.0, 0.0),
						egui::Vec2::splat(INDICATOR_SIZE),
					);
					match row.selected {
						0 => {}
						count if count == row.total => {
							ui.painter().rect_filled(indicator, 0.0, accent);
						}
						_ => draw_partial(ui.painter(), indicator, accent),
					}

					let mut lead = indicator;
					if let Some(open) = row.open {
						lead = egui::Rect::from_center_size(
							indicator.right_center()
								+ egui::vec2(panel::PADDING + ARROW_SIZE / 2.0, 0.0),
							egui::Vec2::splat(ARROW_SIZE),
						);
						let response = ui.interact(
							lead.expand(ICON_GAP),
							ui.id().with(("Expand", item)),
							egui::Sense::click(),
						);
						let tint = if response.hovered() {
							panel::CONTENT_ACTIVE_COLOR
						} else {
							color
						};
						draw_arrow(ui.painter(), lead, open, tint);
						if response.clicked() {
							toggled = Some(row.group);
						}
					}

					let delete_rect = egui::Rect::from_center_size(
						rect.right_center()
							- egui::vec2(panel::PADDING + panel::ICON_SIZE / 2.0, 0.0),
						egui::Vec2::splat(panel::ICON_SIZE),
					);
					let edit_rect =
						delete_rect.translate(egui::vec2(-panel::ICON_SIZE - ICON_GAP, 0.0));
					if icon_button(
						ui,
						&mut self.delete_icon,
						delete_rect,
						color,
						"Delete",
						item,
					)
					.clicked()
					{
						deleted = Some(item);
					}

					if icon_button(ui, &mut self.edit_icon, edit_rect, color, "Rename", item)
						.clicked()
					{
						self.editing = Some((item, row.name.clone()));
					}

					let mut text_end = edit_rect.left();
					if let Item::Group(group) = item {
						let mirror_rect =
							edit_rect.translate(egui::vec2(-panel::ICON_SIZE - ICON_GAP, 0.0));
						let active = row.mirror || edit.mirror_menu_group() == Some(group);
						let tint = if active {
							panel::CONTENT_ACTIVE_COLOR
						} else {
							color
						};
						if icon_button(ui, &mut self.mirror_icon, mirror_rect, tint, "Mirror", item)
							.clicked()
						{
							mirrored = Some(group);
						}
						text_end = mirror_rect.left();
					}

					match &mut self.editing {
						Some((editing, text)) if *editing == item => {
							let text_rect = egui::Rect::from_x_y_ranges(
								lead.right() + panel::PADDING..=text_end - ICON_GAP,
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
									renamed = Some((item, text.trim().to_string()));
								}
								self.editing = None;
							}
						}
						_ => panel::paint_label(ui, lead, &row.label, color),
					}
					rects.push(rect);
				}

				let Some((from, stopped)) = dragged else {
					return;
				};
				let Some(pointer) = ctx.pointer_latest_pos() else {
					return;
				};

				let target = rects
					.iter()
					.filter(|row| row.center().y < pointer.y)
					.count();
				if stopped {
					let (index, group) = match rows.get(target) {
						Some(row) if row.open.is_none() => (row.start, row.group),
						Some(row) => (row.start, 0),
						None => (mesh.layers.len(), 0),
					};
					moved = Some((rows[from].item, index, group));
					return;
				}

				let y = match target {
					0 => rects[0].top(),
					_ => rects[target - 1].bottom(),
				};
				ui.painter()
					.hline(rects[0].x_range(), y, egui::Stroke::new(2.0, DROP_COLOR));
			});

		if let Some(group) = toggled
			&& !self.expanded.remove(&group)
		{
			self.expanded.insert(group);
		}

		let members = |mesh: &Mesh, item: Item| match item {
			Item::Layer(id) => vec![id],
			Item::Group(group) => mesh
				.layers
				.iter()
				.filter(|layer| layer.group == group)
				.map(|layer| layer.id)
				.collect(),
		};

		if let Some(group) = mirrored
			&& let Some(pointer) = ctx.pointer_latest_pos()
		{
			edit.toggle_mirror_menu(mesh, group, pointer);
		}

		match moved {
			Some((Item::Layer(id), target, group)) => edit.move_layer(mesh, id, target, group),
			Some((Item::Group(group), target, _)) => edit.move_group(mesh, group, target),
			None => {}
		}

		match renamed {
			Some((Item::Layer(id), name)) => edit.rename_layer(mesh, id, name),
			Some((Item::Group(group), name)) => edit.rename_group(mesh, group, name),
			None => {}
		}

		if let Some(item) = deleted {
			let layers = members(mesh, item);
			edit.delete_layers(mesh, &layers);
		}

		if let Some(item) = clicked {
			let layers = members(mesh, item);
			let shift = ctx.input(|input| input.modifiers.shift);
			edit.select_layers(mesh, &layers, shift);
		}
	}

	fn rows(&self, mesh: &Mesh, edit: &EditMode) -> Vec<Row> {
		let mut sizes: HashMap<u32, usize> = HashMap::new();
		for vertex in 0..mesh.vertices.len() {
			*sizes.entry(mesh.layer(vertex)).or_default() += 1;
		}

		let mut selected: HashMap<u32, usize> = HashMap::new();
		for &vertex in edit.selection() {
			*selected.entry(mesh.layer(vertex)).or_default() += 1;
		}

		let mut rows = Vec::new();
		let mut open = true;
		for (index, layer) in mesh.layers.iter().enumerate() {
			let id = layer.id;
			let group = layer.group;
			if group != 0 && (index == 0 || mesh.layers[index - 1].group != group) {
				let members = mesh.layers[index..]
					.iter()
					.take_while(|layer| layer.group == group);
				let count = members
					.clone()
					.map(|layer| selected.get(&layer.id).copied().unwrap_or(0))
					.sum();
				let total = members.map(|layer| sizes[&layer.id]).sum();
				open = count > 0 || self.expanded.contains(&group);
				let name = mesh
					.groups
					.iter()
					.find(|entry| entry.id == group && !entry.name.is_empty())
					.map_or_else(|| format!("Group {group}"), |entry| entry.name.clone());
				rows.push(Row {
					item: Item::Group(group),
					start: index,
					group,
					label: name.clone(),
					name,
					selected: count,
					total,
					open: Some(open),
					mirror: mesh.mirror(group) != [false; 2],
				});
			}

			if group != 0 && !open {
				continue;
			}

			let name = if !layer.name.is_empty() {
				layer.name.clone()
			} else if layer.curve {
				format!("Curve {id}")
			} else {
				format!("Shape {id}")
			};
			let label = if layer.holdout {
				format!("{name} (Holdout)")
			} else {
				name.clone()
			};
			rows.push(Row {
				item: Item::Layer(id),
				start: index,
				group,
				name,
				label,
				selected: selected.get(&id).copied().unwrap_or(0),
				total: sizes[&id],
				open: None,
				mirror: false,
			});
		}
		rows
	}
}

fn icon_button(
	ui: &mut egui::Ui,
	icon: &mut Icon,
	rect: egui::Rect,
	color: egui::Color32,
	hint: &str,
	item: Item,
) -> egui::Response {
	let response = ui
		.interact(rect, ui.id().with((hint, item)), egui::Sense::click())
		.on_hover_text(hint);
	let tint = if response.hovered() {
		panel::CONTENT_ACTIVE_COLOR
	} else {
		color
	};
	panel::paint_icon(ui, icon, rect, tint);
	response
}

fn draw_arrow(painter: &egui::Painter, rect: egui::Rect, open: bool, color: egui::Color32) {
	let points = if open {
		vec![rect.left_top(), rect.right_top(), rect.center_bottom()]
	} else {
		vec![rect.left_top(), rect.right_center(), rect.left_bottom()]
	};
	painter.add(egui::Shape::convex_polygon(
		points,
		color,
		egui::Stroke::NONE,
	));
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
