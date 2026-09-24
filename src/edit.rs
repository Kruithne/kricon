use crate::icon;
use crate::menu::Menu;
use crate::mesh::Mesh;
use crate::view::View;
use eframe::egui::{self, Color32, Key, PointerButton, Pos2, Rect, Stroke, Vec2};

const VERTEX_SIZE: f32 = 6.0;
const VERTEX_HIT_RADIUS: f32 = 8.0;
const EDGE_WIDTH: f32 = 1.5;
const SELECTED_COLOR: Color32 = Color32::WHITE;
const MENU: [(MenuAction, &str, &str); 1] = [(MenuAction::AddVertex, "Add Vertex", icon::BORING)];

#[derive(Clone, Copy)]
enum MenuAction {
	AddVertex,
}

#[derive(Default)]
enum Operation {
	#[default]
	Idle,
	Grab {
		isolate: Option<usize>,
	},
	Translate(Translate),
	Menu {
		pos: Pos2,
	},
}

struct Translate {
	anchor: Pos2,
	original: Vec<Pos2>,
	drag: bool,
	extruded_from: Option<Vec<usize>>,
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
			menu: Menu::new(&MENU),
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
		let Operation::Translate(translate) = std::mem::take(&mut self.operation) else {
			return;
		};

		match translate.extruded_from {
			Some(sources) => {
				mesh.remove_vertices(std::mem::replace(&mut self.selection, sources));
			}
			None => {
				for (&index, &pos) in self.selection.iter().zip(&translate.original) {
					mesh.vertices[index] = pos;
				}
			}
		}
	}

	pub fn show_menu(&mut self, ctx: &egui::Context, mesh: &mut Mesh, view: &View) {
		let Operation::Menu { pos } = self.operation else {
			return;
		};

		let Some(action) = self.menu.show(ctx, view.to_screen(pos)) else {
			return;
		};

		self.operation = Operation::Idle;
		match action {
			MenuAction::AddVertex => self.selection = vec![mesh.add_vertex(pos)],
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
					self.begin_translate(mesh, cursor, false, None);
				} else if key(Key::E) {
					self.extrude(mesh, cursor);
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
					self.begin_translate(mesh, anchor, true, None);
				}
			}
			Operation::Translate(translate) => {
				let delta = cursor - translate.anchor;
				for (&index, &pos) in self.selection.iter().zip(&translate.original) {
					mesh.vertices[index] = pos + delta;
				}

				let (confirm, cancel) = if translate.drag {
					(
						input.pointer.button_released(PointerButton::Secondary),
						key(Key::Escape),
					)
				} else {
					(
						pressed(PointerButton::Primary),
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

	fn extrude(&mut self, mesh: &mut Mesh, cursor: Pos2) {
		if self.selection.is_empty() {
			return;
		}

		let sources = std::mem::take(&mut self.selection);
		for &source in &sources {
			let vertex = mesh.add_vertex(mesh.vertices[source]);
			mesh.add_edge(source, vertex);
			self.selection.push(vertex);
		}

		self.begin_translate(mesh, cursor, false, Some(sources));
	}

	fn begin_translate(
		&mut self,
		mesh: &Mesh,
		anchor: Pos2,
		drag: bool,
		extruded_from: Option<Vec<usize>>,
	) {
		if self.selection.is_empty() {
			return;
		}

		self.operation = Operation::Translate(Translate {
			anchor,
			original: self
				.selection
				.iter()
				.map(|&index| mesh.vertices[index])
				.collect(),
			drag,
			extruded_from,
		});
	}
}
