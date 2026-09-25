use crate::history::History;
use crate::icon;
use crate::menu::Menu;
use crate::mesh::Mesh;
use crate::view::View;
use eframe::egui::{
	self, Color32, Event, Key, PointerButton, Pos2, Rect, Stroke, StrokeKind, Vec2, emath::Rot2,
	pos2, vec2,
};
use std::f32::consts::TAU;

const VERTEX_SIZE: f32 = 6.0;
const VERTEX_HIT_RADIUS: f32 = 8.0;
const ALIGN_RADIUS: f32 = 8.0;
const LINK_PICK_RADIUS: f32 = 32.0;
const EDGE_WIDTH: f32 = 1.5;
const AXIS_WIDTH: f32 = 1.0;
const AXIS_X_COLOR: Color32 = Color32::from_rgb(255, 51, 82);
const AXIS_Y_COLOR: Color32 = Color32::from_rgb(139, 220, 0);
pub const SELECTED_COLOR: Color32 = Color32::WHITE;
const RECT_SIZE: f32 = 2.0;
const CIRCLE_RADIUS: f32 = 1.0;
const CIRCLE_SEGMENTS: usize = 16;
const MIN_CIRCLE_SEGMENTS: usize = 3;
const MAX_CIRCLE_SEGMENTS: usize = 128;
const PRIMITIVE_SCALE_STEP: f32 = 1.1;
const TOOL_COLOR: Color32 = Color32::from_rgb(255, 51, 82);
const TOOL_WIDTH: f32 = 1.5;
const BRUSH_RADIUS: f32 = 24.0;
const BRUSH_STEP: f32 = 4.0;
const MIN_BRUSH_RADIUS: f32 = 4.0;
const MAX_BRUSH_RADIUS: f32 = 256.0;
const CREATE_MENU: [(MenuAction, &str, &str); 3] = [
	(MenuAction::AddVertex, "Add Vertex (V)", icon::BORING),
	(MenuAction::AddRect, "Add Rect", icon::BORING),
	(MenuAction::AddCircle, "Add Circle", icon::BORING),
];
const MERGE_MENU: [(MenuAction, &str, &str); 4] = [
	(
		MenuAction::Merge(MergeTarget::Last),
		"At Last",
		icon::BORING,
	),
	(
		MenuAction::Merge(MergeTarget::Center),
		"At Center",
		icon::BORING,
	),
	(
		MenuAction::Merge(MergeTarget::First),
		"At First",
		icon::BORING,
	),
	(
		MenuAction::Merge(MergeTarget::Cursor),
		"At Cursor",
		icon::BORING,
	),
];

#[derive(Clone, Copy)]
enum MenuAction {
	AddVertex,
	AddRect,
	AddCircle,
	Merge(MergeTarget),
}

#[derive(Clone, Copy)]
enum MergeTarget {
	Last,
	Center,
	First,
	Cursor,
}

#[derive(Clone, Copy)]
enum MenuKind {
	Create,
	Merge,
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
		kind: MenuKind,
	},
	Brush,
	BoxSelect {
		start: Option<Pos2>,
	},
}

#[derive(PartialEq)]
enum TransformKind {
	Translate,
	Rotate,
	Scale,
	Inset(Vec<Vec2>),
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
	X,
	Y,
}

struct Transform {
	kind: TransformKind,
	anchor: Pos2,
	pivot: Pos2,
	original: Vec<Pos2>,
	drag: bool,
	before: Vec<usize>,
	axis: Option<Axis>,
	primitive: bool,
	segments: Option<usize>,
	typed: String,
	guides: [Option<Pos2>; 2],
}

impl Transform {
	fn apply(&self, index: usize, pos: Pos2, cursor: Pos2, snap: bool) -> Pos2 {
		let offset = pos - self.pivot;
		let start = self.anchor - self.pivot;
		let current = cursor - self.pivot;
		let angle = self
			.typed_angle()
			.unwrap_or(current.angle() - start.angle());
		let moved = match (&self.kind, self.axis) {
			(TransformKind::Translate, _) => offset + (cursor - self.anchor),
			(TransformKind::Rotate, None) => Rot2::from_angle(angle) * offset,
			(TransformKind::Rotate, Some(_)) => offset * angle.cos(),
			(TransformKind::Scale, _) if start.length() > f32::EPSILON => {
				offset * (current.length() / start.length())
			}
			(TransformKind::Scale, _) => offset,
			(TransformKind::Inset(directions), _) => {
				offset + directions[index] * (start.length() - current.length())
			}
		};

		let target = self.pivot + moved;
		let target = if snap && self.kind == TransformKind::Translate {
			target.round()
		} else {
			target
		};
		pos + (target - pos) * self.free()
	}

	fn free(&self) -> Vec2 {
		match (self.kind == TransformKind::Rotate, self.axis) {
			(_, None) => Vec2::splat(1.0),
			(false, Some(Axis::X)) | (true, Some(Axis::Y)) => Vec2::X,
			(false, Some(Axis::Y)) | (true, Some(Axis::X)) => Vec2::Y,
		}
	}

	fn typed_angle(&self) -> Option<f32> {
		if self.typed.is_empty() {
			return None;
		}

		let (sign, digits) = match self.typed.strip_prefix('-') {
			Some(digits) => (1.0, digits),
			None => (-1.0, self.typed.as_str()),
		};
		Some(sign * digits.parse::<f32>().unwrap_or(0.0).to_radians())
	}

	fn type_char(&mut self, char: char) {
		if char == '-' {
			self.typed = match self.typed.strip_prefix('-') {
				Some(digits) => digits.to_string(),
				None => format!("-{}", self.typed),
			};
		} else if char.is_ascii_digit() || (char == '.' && !self.typed.contains('.')) {
			self.typed.push(char);
		}
	}

	fn toggle_axis(&mut self, axis: Axis) {
		self.axis = (self.axis != Some(axis)).then_some(axis);
	}
}

pub struct EditMode {
	selection: Vec<usize>,
	operation: Operation,
	create_menu: Menu<MenuAction>,
	merge_menu: Menu<MenuAction>,
	brush_radius: f32,
	history: History,
}

impl EditMode {
	pub fn new() -> Self {
		Self {
			selection: Vec::new(),
			operation: Operation::Idle,
			create_menu: Menu::new("Create", icon::BORING, &CREATE_MENU),
			merge_menu: Menu::new("Merge", icon::BORING, &MERGE_MENU),
			brush_radius: BRUSH_RADIUS,
			history: History::default(),
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

		self.history.revert(mesh);
		self.selection = transform.before;
	}

	pub fn move_layer(&mut self, mesh: &mut Mesh, from: usize, target: usize) {
		self.cancel(mesh);
		self.record(mesh, |_, mesh| mesh.move_layer(from, target));
	}

	pub fn captures_scroll(&self) -> bool {
		matches!(
			self.operation,
			Operation::Transform(Transform {
				segments: Some(_),
				..
			}) | Operation::Brush
		)
	}

	pub fn captures_middle(&self) -> bool {
		matches!(self.operation, Operation::Brush)
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
		let Operation::Menu { pos, kind } = self.operation else {
			return;
		};

		let menu = match kind {
			MenuKind::Create => &mut self.create_menu,
			MenuKind::Merge => &mut self.merge_menu,
		};
		let Some(action) = menu.show(ctx, view.to_screen(pos), accent) else {
			return;
		};

		self.operation = Operation::Idle;
		match action {
			MenuAction::AddVertex => {
				self.record(mesh, |edit, mesh| {
					edit.selection = vec![mesh.add_vertex(pos)]
				});
			}
			MenuAction::AddRect => self.add_primitive(mesh, &rect_points(pos), pos, None),
			MenuAction::AddCircle => self.add_primitive(
				mesh,
				&circle_points(pos, CIRCLE_SEGMENTS, CIRCLE_RADIUS),
				pos,
				Some(CIRCLE_SEGMENTS),
			),
			MenuAction::Merge(target) => {
				self.record(mesh, |edit, mesh| edit.merge(mesh, target, pos));
			}
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

		if let Operation::Transform(transform) = &self.operation {
			if let Some(axis) = transform.axis {
				axis_line(painter, axis, view.to_screen(transform.pivot));
			}

			let [x, y] = transform.guides;
			if let Some(guide) = x {
				axis_line(painter, Axis::Y, view.to_screen(guide));
			}
			if let Some(guide) = y {
				axis_line(painter, Axis::X, view.to_screen(guide));
			}
		}

		if let Operation::Brush = self.operation
			&& let Some(pos) = painter.ctx().pointer_hover_pos()
		{
			painter.circle_stroke(pos, self.brush_radius, Stroke::new(TOOL_WIDTH, TOOL_COLOR));
		}

		if let Operation::BoxSelect { start: Some(start) } = self.operation
			&& let Some(pos) = painter.ctx().pointer_latest_pos()
		{
			painter.rect_stroke(
				Rect::from_two_pos(view.to_screen(start), pos),
				0.0,
				Stroke::new(TOOL_WIDTH, TOOL_COLOR),
				StrokeKind::Middle,
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

		match &mut self.operation {
			Operation::Idle => {
				if key(Key::Z) && input.modifiers.ctrl {
					self.history.undo(mesh, &mut self.selection);
				} else if key(Key::R) && input.modifiers.ctrl {
					self.history.redo(mesh, &mut self.selection);
				} else if key(Key::G) {
					self.begin_transform(
						mesh,
						TransformKind::Translate,
						cursor,
						false,
						self.selection.clone(),
					);
				} else if key(Key::R) {
					self.begin_transform(
						mesh,
						TransformKind::Rotate,
						cursor,
						false,
						self.selection.clone(),
					);
				} else if key(Key::S) && input.modifiers.alt {
					self.record(mesh, |edit, mesh| {
						edit.selection = mesh.decimate(&edit.selection);
					});
				} else if key(Key::S) && (input.modifiers.shift || input.modifiers.ctrl) {
					self.record(mesh, |edit, mesh| {
						let midpoints = mesh.subdivide(&edit.selection, input.modifiers.ctrl);
						edit.extend_selection(midpoints);
					});
				} else if key(Key::S) {
					self.begin_transform(
						mesh,
						TransformKind::Scale,
						cursor,
						false,
						self.selection.clone(),
					);
				} else if key(Key::F) {
					if let [a, b] = self.selection[..] {
						self.record(mesh, |_, mesh| mesh.add_edge(a, b));
					}
				} else if key(Key::D) && input.modifiers.shift {
					self.create(mesh, cursor, Mesh::duplicate);
				} else if key(Key::E) {
					self.create(mesh, cursor, Mesh::extrude);
				} else if key(Key::I) {
					self.inset(mesh, cursor);
				} else if key(Key::V) {
					self.record(mesh, |edit, mesh| {
						edit.selection = vec![mesh.add_vertex(cursor)]
					});
				} else if key(Key::L) {
					if self.selection.is_empty() {
						let radius = LINK_PICK_RADIUS / view.scale;
						self.selection.extend(mesh.nearest_vertex(cursor, radius));
					}
					self.extend_selection(mesh.linked(&self.selection));
				} else if key(Key::W) {
					self.operation = Operation::Menu {
						pos: cursor,
						kind: MenuKind::Create,
					};
				} else if key(Key::M) {
					self.operation = Operation::Menu {
						pos: cursor,
						kind: MenuKind::Merge,
					};
				} else if key(Key::X) {
					self.record(mesh, |edit, mesh| {
						mesh.dissolve(std::mem::take(&mut edit.selection));
					});
				} else if key(Key::P) {
					self.record(mesh, |edit, mesh| mesh.toggle_hole(&edit.selection));
				} else if key(Key::C) {
					self.operation = Operation::Brush;
				} else if key(Key::B) {
					self.operation = Operation::BoxSelect { start: None };
				} else if key(Key::Delete) {
					self.record(mesh, |edit, mesh| {
						mesh.remove_vertices(std::mem::take(&mut edit.selection));
					});
				} else if hovered && pressed(PointerButton::Secondary) {
					let radius = VERTEX_HIT_RADIUS / view.scale;
					let hit = mesh.nearest_vertex(cursor, radius);
					if hit.is_none()
						&& let Some(face) = mesh.face_at(cursor)
					{
						self.select_face(face, input.modifiers.shift);
					} else {
						self.select(hit, input.modifiers.shift);
					}
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
					self.begin_transform(
						mesh,
						TransformKind::Translate,
						anchor,
						true,
						self.selection.clone(),
					);
				}
			}
			Operation::Transform(transform) => {
				if key(Key::X) {
					transform.toggle_axis(Axis::X);
				} else if key(Key::Y) {
					transform.toggle_axis(Axis::Y);
				} else if key(Key::Backspace) {
					transform.typed.pop();
				}

				if keyboard && transform.kind == TransformKind::Rotate {
					for event in &input.events {
						if let Event::Text(text) = event {
							text.chars().for_each(|char| transform.type_char(char));
						}
					}
				}

				let steps = wheel_steps(input);
				if transform.primitive && input.modifiers.shift && steps != 0 {
					let factor = PRIMITIVE_SCALE_STEP.powi(steps as i32);
					for pos in &mut transform.original {
						*pos = transform.pivot + (*pos - transform.pivot) * factor;
					}
				} else if let Some(segments) = &mut transform.segments
					&& steps != 0
				{
					*segments = segments
						.saturating_add_signed(steps)
						.clamp(MIN_CIRCLE_SEGMENTS, MAX_CIRCLE_SEGMENTS);
					let radius = transform.original[0].distance(transform.pivot);
					mesh.remove_vertices(std::mem::take(&mut self.selection));
					self.selection =
						mesh.add_loop(&circle_points(transform.pivot, *segments, radius));
					transform.original = self
						.selection
						.iter()
						.map(|&index| mesh.vertices[index])
						.collect();
				}

				for (index, (&vertex, &pos)) in
					self.selection.iter().zip(&transform.original).enumerate()
				{
					mesh.vertices[vertex] =
						transform.apply(index, pos, cursor, input.modifiers.alt);
				}

				transform.guides = if input.modifiers.shift
					&& !transform.primitive
					&& transform.kind == TransformKind::Translate
				{
					align(
						mesh,
						&self.selection,
						transform.free(),
						ALIGN_RADIUS / view.scale,
					)
				} else {
					[None; 2]
				};

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
					self.confirm(mesh);
				}
			}
			Operation::Brush => {
				let steps = wheel_steps(input) as f32;
				self.brush_radius = (self.brush_radius + steps * BRUSH_STEP)
					.clamp(MIN_BRUSH_RADIUS, MAX_BRUSH_RADIUS);

				let down = |button: PointerButton| hovered && input.pointer.button_down(button);
				let inside = mesh.vertices_within(cursor, self.brush_radius / view.scale);
				if key(Key::C) || key(Key::Escape) || (hovered && pressed(PointerButton::Secondary))
				{
					self.operation = Operation::Idle;
				} else if down(PointerButton::Primary) {
					self.extend_selection(inside);
				} else if down(PointerButton::Middle) {
					let inside: Vec<usize> = inside.collect();
					self.selection.retain(|vertex| !inside.contains(vertex));
				}
			}
			Operation::BoxSelect { start } => {
				if key(Key::B) || key(Key::Escape) || (hovered && pressed(PointerButton::Secondary))
				{
					self.operation = Operation::Idle;
				} else if let Some(start) = *start {
					if !input.pointer.button_down(PointerButton::Primary) {
						let rect = Rect::from_two_pos(start, cursor);
						self.extend_selection(mesh.vertices_in(rect));
						self.operation = Operation::Idle;
					}
				} else if hovered && pressed(PointerButton::Primary) {
					*start = Some(cursor);
				}
			}
			Operation::Menu { .. } => {
				if key(Key::Escape) || (hovered && input.pointer.any_pressed()) {
					self.operation = Operation::Idle;
				}
			}
		}
	}

	fn confirm(&mut self, mesh: &Mesh) {
		let Operation::Transform(transform) = std::mem::take(&mut self.operation) else {
			return;
		};

		self.history.commit(mesh, transform.before, &self.selection);
	}

	fn record(&mut self, mesh: &mut Mesh, action: impl FnOnce(&mut Self, &mut Mesh)) {
		let before = self.selection.clone();
		action(self, mesh);
		self.history.commit(mesh, before, &self.selection);
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

	fn select_face(&mut self, face: Vec<usize>, shift: bool) {
		if shift && face.iter().all(|vertex| self.selection.contains(vertex)) {
			self.selection.retain(|vertex| !face.contains(vertex));
			return;
		}

		if !shift {
			self.selection.clear();
		}

		self.extend_selection(face);
		self.operation = Operation::Grab { isolate: None };
	}

	fn extend_selection(&mut self, vertices: impl IntoIterator<Item = usize>) {
		for vertex in vertices {
			if !self.selection.contains(&vertex) {
				self.selection.push(vertex);
			}
		}
	}

	fn create(
		&mut self,
		mesh: &mut Mesh,
		cursor: Pos2,
		create: fn(&mut Mesh, &[usize]) -> Vec<usize>,
	) {
		if self.selection.is_empty() {
			return;
		}

		let sources = std::mem::take(&mut self.selection);
		self.selection = create(mesh, &sources);
		self.begin_transform(mesh, TransformKind::Translate, cursor, false, sources);
	}

	fn inset(&mut self, mesh: &mut Mesh, cursor: Pos2) {
		let (inner, directions) = mesh.inset(&self.selection);
		if inner.is_empty() {
			return;
		}

		let sources = std::mem::replace(&mut self.selection, inner);
		self.begin_transform(
			mesh,
			TransformKind::Inset(directions),
			cursor,
			false,
			sources,
		);
	}

	fn add_primitive(
		&mut self,
		mesh: &mut Mesh,
		points: &[Pos2],
		cursor: Pos2,
		segments: Option<usize>,
	) {
		let sources = std::mem::replace(&mut self.selection, mesh.add_loop(points));
		self.begin_transform(mesh, TransformKind::Translate, cursor, false, sources);
		if let Operation::Transform(transform) = &mut self.operation {
			transform.primitive = true;
			transform.segments = segments;
		}
	}

	fn merge(&mut self, mesh: &mut Mesh, target: MergeTarget, cursor: Pos2) {
		let (Some(&first), Some(&last)) = (self.selection.first(), self.selection.last()) else {
			return;
		};

		let pos = match target {
			MergeTarget::Last => mesh.vertices[last],
			MergeTarget::Center => center(mesh, &self.selection),
			MergeTarget::First => mesh.vertices[first],
			MergeTarget::Cursor => cursor,
		};
		self.selection = vec![mesh.merge(&self.selection, pos)];
	}

	fn begin_transform(
		&mut self,
		mesh: &Mesh,
		kind: TransformKind,
		anchor: Pos2,
		drag: bool,
		before: Vec<usize>,
	) {
		if self.selection.is_empty() {
			return;
		}

		let original: Vec<Pos2> = self
			.selection
			.iter()
			.map(|&index| mesh.vertices[index])
			.collect();

		self.operation = Operation::Transform(Transform {
			kind,
			anchor,
			pivot: center(mesh, &self.selection),
			original,
			drag,
			before,
			axis: None,
			primitive: false,
			segments: None,
			typed: String::new(),
			guides: [None; 2],
		});
	}
}

fn center(mesh: &Mesh, vertices: &[usize]) -> Pos2 {
	let sum = vertices.iter().fold(Vec2::ZERO, |sum, &index| {
		sum + mesh.vertices[index].to_vec2()
	});
	(sum / vertices.len() as f32).to_pos2()
}

fn align(mesh: &mut Mesh, selection: &[usize], free: Vec2, radius: f32) -> [Option<Pos2>; 2] {
	let mut selected = vec![false; mesh.vertices.len()];
	for &index in selection {
		selected[index] = true;
	}

	let mut best: [Option<(f32, Pos2)>; 2] = [None; 2];
	for (index, &target) in mesh.vertices.iter().enumerate() {
		if selected[index] {
			continue;
		}

		for &vertex in selection {
			let pos = mesh.vertices[vertex];
			for axis in 0..2 {
				let delta = target[axis] - pos[axis];
				if free[axis] > 0.0
					&& delta.abs() < radius
					&& best[axis].is_none_or(|(best, _)| delta.abs() < best.abs())
				{
					best[axis] = Some((delta, target));
				}
			}
		}
	}

	let offset = best.map(|best| best.map_or(0.0, |(delta, _)| delta));
	for &vertex in selection {
		mesh.vertices[vertex] += vec2(offset[0], offset[1]);
	}
	best.map(|best| best.map(|(_, target)| target))
}

fn axis_line(painter: &egui::Painter, axis: Axis, pos: Pos2) {
	let clip = painter.clip_rect();
	let (color, points) = match axis {
		Axis::X => (
			AXIS_X_COLOR,
			[pos2(clip.left(), pos.y), pos2(clip.right(), pos.y)],
		),
		Axis::Y => (
			AXIS_Y_COLOR,
			[pos2(pos.x, clip.top()), pos2(pos.x, clip.bottom())],
		),
	};
	painter.line_segment(points, Stroke::new(AXIS_WIDTH, color));
}

fn wheel_steps(input: &egui::InputState) -> isize {
	input
		.events
		.iter()
		.filter_map(|event| match event {
			Event::MouseWheel { delta, .. } if delta.y != 0.0 => Some(delta.y.signum() as isize),
			_ => None,
		})
		.sum()
}

fn rect_points(center: Pos2) -> [Pos2; 4] {
	let half = RECT_SIZE / 2.0;
	[
		center + vec2(-half, -half),
		center + vec2(half, -half),
		center + vec2(half, half),
		center + vec2(-half, half),
	]
}

fn circle_points(center: Pos2, segments: usize, radius: f32) -> Vec<Pos2> {
	(0..segments)
		.map(|index| {
			let angle = TAU * index as f32 / segments as f32;
			center + radius * vec2(angle.cos(), angle.sin())
		})
		.collect()
}
