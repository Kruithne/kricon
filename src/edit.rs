use crate::icon;
use crate::menu::Menu;
use crate::mesh::Mesh;
use crate::view::View;
use eframe::egui::{self, Color32, Key, PointerButton, Pos2, Rect, Stroke, Vec2, emath::Rot2};

const VERTEX_SIZE: f32 = 6.0;
const VERTEX_HIT_RADIUS: f32 = 8.0;
const LINK_PICK_RADIUS: f32 = 32.0;
const EDGE_WIDTH: f32 = 1.5;
pub const SELECTED_COLOR: Color32 = Color32::WHITE;
const MENU: [(MenuAction, &str, &str); 2] = [
	(MenuAction::AddVertex, "Add Vertex (V)", icon::BORING),
	(MenuAction::SelectLinked, "Select Linked (L)", icon::BORING),
];

#[derive(Clone, Copy)]
enum MenuAction {
	AddVertex,
	SelectLinked,
}

#[derive(Default)]
enum Operation {
	#[default]
	Idle,
	Grab {
		isolate: Option<usize>,
	},
	Transform(Transform),
	Menu {
		pos: Pos2,
	},
}

enum TransformKind {
	Translate,
	Rotate,
	Scale,
}

struct Transform {
	kind: TransformKind,
	anchor: Pos2,
	pivot: Pos2,
	original: Vec<Pos2>,
	drag: bool,
	created_from: Option<Vec<usize>>,
}

impl Transform {
	fn apply(&self, pos: Pos2, cursor: Pos2) -> Pos2 {
		let start = self.anchor - self.pivot;
		let current = cursor - self.pivot;
		match self.kind {
			TransformKind::Translate => pos + (cursor - self.anchor),
			TransformKind::Rotate => {
				self.pivot + Rot2::from_angle(current.angle() - start.angle()) * (pos - self.pivot)
			}
			TransformKind::Scale => {
				let factor = if start.length() > f32::EPSILON {
					current.length() / start.length()
				} else {
					1.0
				};
				self.pivot + (pos - self.pivot) * factor
			}
		}
	}
}

pub struct EditMode {
	selection: Vec<usize>,
	operation: Operation,
	menu: Menu<MenuAction>,
}

impl EditMode {
	pub fn new() -> Self {
		Self {
			selection: Vec::new(),
			operation: Operation::Idle,
			menu: Menu::new("Quick Menu", icon::BORING, &MENU),
		}
	}

	pub fn update(&mut self, mesh: &mut Mesh, view: &View, response: &egui::Response) {
		let keyboard = !response.ctx.egui_wants_keyboard_input();
		let hovered = response.hovered();
		response
			.ctx
			.input(|input| self.handle_input(mesh, view, input, hovered, keyboard));
	}

	pub fn cancel(&mut self, mesh: &mut Mesh) {
		let Operation::Transform(transform) = std::mem::take(&mut self.operation) else {
			return;
		};

		match transform.created_from {
			Some(sources) => {
				mesh.remove_vertices(std::mem::replace(&mut self.selection, sources));
			}
			None => {
				for (&index, &pos) in self.selection.iter().zip(&transform.original) {
					mesh.vertices[index] = pos;
				}
			}
		}
	}

	pub fn selection(&self) -> &[usize] {
		&self.selection
	}

	pub fn select_layer(&mut self, mesh: &mut Mesh, layer: u32, shift: bool) {
		self.cancel(mesh);
		self.operation = Operation::Idle;

		let deselect = shift
			&& self
				.selection
				.iter()
				.any(|&vertex| mesh.layer(vertex) == layer);
		if !shift {
			self.selection.clear();
		}

		self.selection.retain(|&vertex| mesh.layer(vertex) != layer);
		if !deselect {
			self.selection.extend(mesh.layer_vertices(layer));
		}
	}

	pub fn show_menu(
		&mut self,
		ctx: &egui::Context,
		mesh: &mut Mesh,
		view: &View,
		accent: Color32,
	) {
		let Operation::Menu { pos } = self.operation else {
			return;
		};

		let Some(action) = self.menu.show(ctx, view.to_screen(pos), accent) else {
			return;
		};

		self.operation = Operation::Idle;
		match action {
			MenuAction::AddVertex => self.selection = vec![mesh.add_vertex(pos)],
			MenuAction::SelectLinked => self.select_linked(mesh),
		}
	}

	pub fn draw(&self, mesh: &Mesh, view: &View, painter: &egui::Painter, accent: Color32) {
		let mut selected = vec![false; mesh.vertices.len()];
		for &index in &self.selection {
			selected[index] = true;
		}

		let color = |is_selected: bool| if is_selected { SELECTED_COLOR } else { accent };

		for &[a, b] in &mesh.edges {
			let points = [
				view.to_screen(mesh.vertices[a]),
				view.to_screen(mesh.vertices[b]),
			];
			painter.line_segment(
				points,
				Stroke::new(EDGE_WIDTH, color(selected[a] && selected[b])),
			);
		}

		for (index, &vertex) in mesh.vertices.iter().enumerate() {
			let rect = Rect::from_center_size(view.to_screen(vertex), Vec2::splat(VERTEX_SIZE));
			painter.rect_filled(rect, 0.0, color(selected[index]));
		}
	}

	fn handle_input(
		&mut self,
		mesh: &mut Mesh,
		view: &View,
		input: &egui::InputState,
		hovered: bool,
		keyboard: bool,
	) {
		let Some(cursor) = input.pointer.latest_pos().map(|pos| view.to_world(pos)) else {
			return;
		};

		let key = |key: Key| keyboard && input.key_pressed(key);
		let pressed = |button: PointerButton| input.pointer.button_pressed(button);

		match &self.operation {
			Operation::Idle => {
				if key(Key::G) {
					self.begin_transform(mesh, TransformKind::Translate, cursor, false, None);
				} else if key(Key::R) {
					self.begin_transform(mesh, TransformKind::Rotate, cursor, false, None);
				} else if key(Key::S) {
					self.begin_transform(mesh, TransformKind::Scale, cursor, false, None);
				} else if key(Key::F) {
					if let [a, b] = self.selection[..] {
						mesh.add_edge(a, b);
					}
				} else if key(Key::D) && input.modifiers.shift {
					self.duplicate(mesh, cursor);
				} else if key(Key::E) {
					self.extrude(mesh, cursor);
				} else if key(Key::V) {
					self.selection = vec![mesh.add_vertex(cursor)];
				} else if key(Key::L) {
					if self.selection.is_empty() {
						let radius = LINK_PICK_RADIUS / view.scale;
						self.selection.extend(mesh.nearest_vertex(cursor, radius));
					}
					self.select_linked(mesh);
				} else if key(Key::W) {
					self.operation = Operation::Menu { pos: cursor };
				} else if key(Key::Delete) {
					mesh.remove_vertices(std::mem::take(&mut self.selection));
				} else if hovered && pressed(PointerButton::Secondary) {
					let radius = VERTEX_HIT_RADIUS / view.scale;
					self.select(mesh.nearest_vertex(cursor, radius), input.modifiers.shift);
				}
			}
			Operation::Grab { isolate } => {
				if !input.pointer.button_down(PointerButton::Secondary) {
					if let Some(index) = *isolate {
						self.selection = vec![index];
					}
					self.operation = Operation::Idle;
				} else if input.pointer.is_decidedly_dragging() {
					let anchor = input
						.pointer
						.press_origin()
						.map_or(cursor, |pos| view.to_world(pos));
					self.begin_transform(mesh, TransformKind::Translate, anchor, true, None);
				}
			}
			Operation::Transform(transform) => {
				for (&index, &pos) in self.selection.iter().zip(&transform.original) {
					mesh.vertices[index] = transform.apply(pos, cursor);
				}

				let (confirm, cancel) = if transform.drag {
					(
						input.pointer.button_released(PointerButton::Secondary) || key(Key::Enter),
						key(Key::Escape),
					)
				} else {
					(
						pressed(PointerButton::Primary) || key(Key::Enter),
						key(Key::Escape) || pressed(PointerButton::Secondary),
					)
				};

				if cancel {
					self.cancel(mesh);
				} else if confirm {
					self.operation = Operation::Idle;
				}
			}
			Operation::Menu { .. } => {
				if key(Key::Escape) || (hovered && input.pointer.any_pressed()) {
					self.operation = Operation::Idle;
				}
			}
		}
	}

	fn select(&mut self, hit: Option<usize>, shift: bool) {
		let Some(index) = hit else {
			if !shift {
				self.selection.clear();
			}
			return;
		};

		let position = self
			.selection
			.iter()
			.position(|&selected| selected == index);
		match (position, shift) {
			(Some(position), true) => {
				self.selection.remove(position);
				return;
			}
			(None, true) => self.selection.push(index),
			(None, false) => self.selection = vec![index],
			(Some(_), false) => {}
		}

		let isolate = (!shift && self.selection.len() > 1).then_some(index);
		self.operation = Operation::Grab { isolate };
	}

	fn select_linked(&mut self, mesh: &Mesh) {
		for vertex in mesh.linked(&self.selection) {
			if !self.selection.contains(&vertex) {
				self.selection.push(vertex);
			}
		}
	}

	fn extrude(&mut self, mesh: &mut Mesh, cursor: Pos2) {
		if self.selection.is_empty() {
			return;
		}

		let sources = std::mem::take(&mut self.selection);
		for &source in &sources {
			self.selection.push(mesh.extrude_vertex(source));
		}

		self.begin_transform(mesh, TransformKind::Translate, cursor, false, Some(sources));
	}

	fn duplicate(&mut self, mesh: &mut Mesh, cursor: Pos2) {
		if self.selection.is_empty() {
			return;
		}

		let sources = std::mem::take(&mut self.selection);
		self.selection = mesh.duplicate(&sources);
		self.begin_transform(mesh, TransformKind::Translate, cursor, false, Some(sources));
	}

	fn begin_transform(
		&mut self,
		mesh: &Mesh,
		kind: TransformKind,
		anchor: Pos2,
		drag: bool,
		created_from: Option<Vec<usize>>,
	) {
		if self.selection.is_empty() {
			return;
		}

		let original: Vec<Pos2> = self
			.selection
			.iter()
			.map(|&index| mesh.vertices[index])
			.collect();
		let sum = original
			.iter()
			.fold(Vec2::ZERO, |sum, pos| sum + pos.to_vec2());
		let pivot = (sum / original.len() as f32).to_pos2();

		self.operation = Operation::Transform(Transform {
			kind,
			anchor,
			pivot,
			original,
			drag,
			created_from,
		});
	}
}
