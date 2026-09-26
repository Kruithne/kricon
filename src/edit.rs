use crate::export;
use crate::geometry::fillet;
use crate::history::History;
use crate::icon;
use crate::images::Image;
use crate::import::{self, Shape};
use crate::menu::Menu;
use crate::mesh::{Corner, Mesh};
use crate::panel;
use crate::view::View;
use eframe::egui::{
	self, Color32, Event, Key, PointerButton, Pos2, Rect, Stroke, StrokeKind, Vec2,
	color_picker::{self, Alpha},
	ecolor::HexColor,
	emath::Rot2,
	vec2,
};
use std::collections::HashSet;
use std::f32::consts::TAU;

const VERTEX_SIZE: f32 = 6.0;
const SHARP_SIZE: f32 = 9.0;
const VERTEX_HIT_RADIUS: f32 = 16.0;
const ALIGN_RADIUS: f32 = 8.0;
const LINK_PICK_RADIUS: f32 = 32.0;
const EDGE_WIDTH: f32 = 1.5;
const AXIS_WIDTH: f32 = 1.0;
const AXIS_X_COLOR: Color32 = Color32::from_rgb(255, 51, 82);
const AXIS_Y_COLOR: Color32 = Color32::from_rgb(139, 220, 0);
const AXIS_NORMAL_COLOR: Color32 = Color32::from_rgb(40, 144, 255);
pub const SELECTED_COLOR: Color32 = Color32::WHITE;
const RECT_SIZE: f32 = 2.0;
const CIRCLE_RADIUS: f32 = 1.0;
const CIRCLE_SEGMENTS: usize = 16;
const MIN_CIRCLE_SEGMENTS: usize = 3;
const MAX_CIRCLE_SEGMENTS: usize = 128;
const CURVE_POINTS: usize = 8;
const CAGE_OPACITY: f32 = 0.4;
const PRIMITIVE_SCALE_STEP: f32 = 1.1;
const TOOL_COLOR: Color32 = Color32::from_rgb(255, 51, 82);
const TOOL_WIDTH: f32 = 1.5;
const BRUSH_RADIUS: f32 = 24.0;
const BRUSH_STEP: f32 = 4.0;
const MIN_BRUSH_RADIUS: f32 = 4.0;
const MAX_BRUSH_RADIUS: f32 = 256.0;
const MAGNET_RADIUS: f32 = 64.0;
const MIN_BEVEL_SEGMENTS: usize = 1;
const MAX_BEVEL_SEGMENTS: usize = 64;
const CREATE_MENU: [(Action, &str); 4] = [
	(Action::AddVertex, "Add Vertex (V)"),
	(Action::AddRect, "Add Rect"),
	(Action::AddCircle, "Add Circle"),
	(Action::AddCurve, "Add Curve"),
];
const MERGE_MENU: [(Action, &str); 4] = [
	(Action::Merge(MergeTarget::Last), "At Last"),
	(Action::Merge(MergeTarget::Center), "At Center"),
	(Action::Merge(MergeTarget::First), "At First"),
	(Action::Merge(MergeTarget::Cursor), "At Cursor"),
];
const MIRROR_MENU: [(Action, &str); 2] =
	[(Action::Mirror(0), "X Axis"), (Action::Mirror(1), "Y Axis")];
const FILE_MENU: [(Action, &str); 3] = [
	(Action::File(FileAction::New), "New Workspace"),
	(Action::File(FileAction::Load), "Load Workspace"),
	(Action::File(FileAction::Save), "Save Workspace (Ctrl+S)"),
];
const MAIN_MENU: [(Action, &str); 31] = [
	(Action::Translate, "Translate (G)"),
	(Action::Rotate, "Rotate (R)"),
	(Action::Scale, "Scale (S)"),
	(Action::Extrude, "Extrude (E)"),
	(Action::Duplicate, "Duplicate (Shift+D)"),
	(Action::Inset, "Inset (I)"),
	(Action::Bevel, "Bevel (Ctrl+B)"),
	(Action::Connect, "Connect (F)"),
	(Action::Subdivide, "Subdivide (Shift+S)"),
	(Action::SubdivideCurve, "Subdivide Curve (Alt+S)"),
	(Action::Decimate, "Decimate (D)"),
	(Action::Space, "Space Evenly (N)"),
	(Action::Dissolve, "Dissolve (X)"),
	(Action::Delete, "Delete (Del)"),
	(Action::SelectAll, "Select All (A)"),
	(Action::SelectLinked, "Select Linked (L)"),
	(Action::SelectSameEdge, "Select Same Edge (T)"),
	(Action::Brush, "Brush Select (C)"),
	(Action::BoxSelect, "Box Select (B)"),
	(Action::Magnet, "Toggle Magnet (K)"),
	(Action::ToggleHole, "Toggle Hole (P)"),
	(Action::ToggleHoldout, "Toggle Holdout (H)"),
	(Action::ToggleSharp, "Toggle Sharp (U)"),
	(Action::Group, "Group (Ctrl+G)"),
	(Action::Ungroup, "Ungroup (Ctrl+Shift+G)"),
	(Action::Palette, "Set Colour (Y)"),
	(Action::CopyColor, "Copy Colour (Ctrl+Y)"),
	(Action::PasteColor, "Paste Colour (Shift+Y)"),
	(Action::CopySvg, "Copy SVG Code (Ctrl+C)"),
	(Action::Undo, "Undo (Ctrl+Z)"),
	(Action::Redo, "Redo (Ctrl+R)"),
];

#[derive(Clone, Copy, PartialEq)]
pub enum FileAction {
	New,
	Load,
	Save,
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
	File(FileAction),
	Undo,
	Redo,
	Translate,
	Rotate,
	Scale,
	Subdivide,
	SubdivideCurve,
	Decimate,
	Space,
	Extrude,
	Duplicate,
	Inset,
	Bevel,
	Connect,
	AddVertex,
	AddRect,
	AddCircle,
	AddCurve,
	SelectAll,
	SelectLinked,
	SelectSameEdge,
	Brush,
	BoxSelect,
	Magnet,
	ToggleOutlines,
	ToggleImages,
	Menu(MenuKind),
	MainMenu,
	Merge(MergeTarget),
	Dissolve,
	ToggleHole,
	ToggleHoldout,
	ToggleSharp,
	Group,
	Ungroup,
	Mirror(usize),
	Palette,
	CopyColor,
	PasteColor,
	CopySvg,
	Delete,
}

#[derive(Clone, Copy, PartialEq)]
enum MergeTarget {
	Last,
	Center,
	First,
	Cursor,
}

#[derive(Clone, Copy, PartialEq)]
enum MenuKind {
	Create,
	Merge,
}

#[derive(Default)]
enum Operation {
	#[default]
	Idle,
	Grab {
		isolate: Option<Selection>,
	},
	Transform(Transform),
	Bevel(Bevel),
	Menu {
		pos: Pos2,
		kind: MenuKind,
	},
	MainMenu {
		anchor: Pos2,
	},
	MirrorMenu {
		anchor: Pos2,
		group: u32,
	},
	Brush,
	BoxSelect {
		start: Option<Pos2>,
	},
	Palette {
		pos: Pos2,
		color: Color32,
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
	Normal,
}

struct Transform {
	kind: TransformKind,
	anchor: Pos2,
	pivot: Pos2,
	original: Vec<Pos2>,
	drag: bool,
	before: Selection,
	axis: Option<Axis>,
	normal: Option<Vec2>,
	primitive: bool,
	segments: Option<usize>,
	typed: String,
	guides: [Option<Pos2>; 2],
	magnet: Option<Vec<(usize, Pos2)>>,
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
		pos + self.constrain(target - pos)
	}

	fn direction(&self, axis: Axis) -> Option<Vec2> {
		match axis {
			Axis::X => Some(Vec2::X),
			Axis::Y => Some(Vec2::Y),
			Axis::Normal => self.normal,
		}
	}

	fn line(&self) -> Option<Vec2> {
		let direction = self.direction(self.axis?)?;
		match self.kind {
			TransformKind::Rotate => Some(direction.rot90()),
			_ => Some(direction),
		}
	}

	fn constrain(&self, delta: Vec2) -> Vec2 {
		self.line().map_or(delta, |line| line * delta.dot(line))
	}

	fn free(&self) -> Vec2 {
		self.line().map_or(Vec2::splat(1.0), Vec2::abs)
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

	fn type_char(&mut self, symbol: char) {
		if symbol == '-' {
			self.typed = match self.typed.strip_prefix('-') {
				Some(digits) => digits.to_string(),
				None => format!("-{}", self.typed),
			};
		} else if symbol.is_ascii_digit() || (symbol == '.' && !self.typed.contains('.')) {
			self.typed.push(symbol);
		}
	}

	fn toggle_axis(&mut self, axis: Axis) {
		if self.direction(axis).is_none() {
			return;
		}

		self.axis = (self.axis != Some(axis)).then_some(axis);
	}
}

struct Bevel {
	anchor: Pos2,
	pivot: Pos2,
	corners: Vec<Corner>,
	segments: usize,
	before: Selection,
}

impl Bevel {
	fn apply(&self, mesh: &mut Mesh, cursor: Pos2) {
		let offset = (cursor.distance(self.pivot) - self.anchor.distance(self.pivot)).max(0.0);
		for corner in &self.corners {
			let distance = offset.min(corner.limit);
			for (index, &vertex) in corner.chain.iter().enumerate() {
				let t = index as f32 / self.segments as f32;
				mesh.vertices[vertex] = fillet(corner.pos, corner.ends, distance, t);
			}
		}
	}

	fn selection(&self) -> Selection {
		let mut selection = self.before.clone();
		for corner in &self.corners {
			selection.vertices.extend(&corner.chain[1..]);
		}
		selection
	}
}

#[derive(Clone, Default)]
pub struct Selection {
	vertices: Vec<usize>,
	images: Vec<usize>,
}

pub struct EditMode {
	pub show_images: bool,
	pub show_outlines: bool,
	selection: Selection,
	operation: Operation,
	create_menu: Menu<Action>,
	merge_menu: Menu<Action>,
	main_menu: Menu<Action>,
	mirror_menu: Menu<Action>,
	brush_radius: f32,
	magnet: bool,
	magnet_radius: f32,
	history: History,
	paste_pending: bool,
	image_paste: bool,
	request: Option<FileAction>,
	pointer: Option<Pos2>,
}

impl Default for EditMode {
	fn default() -> Self {
		Self {
			show_images: true,
			show_outlines: true,
			selection: Selection::default(),
			operation: Operation::Idle,
			create_menu: Menu::new("Create", None).items(&CREATE_MENU),
			merge_menu: Menu::new("Merge", None).items(&MERGE_MENU),
			main_menu: Menu::new("Menu (Q)", Some(icon::MENU))
				.items(&FILE_MENU)
				.submenu("Create (W)", Menu::new("Create", None).items(&CREATE_MENU))
				.submenu("Merge (M)", Menu::new("Merge", None).items(&MERGE_MENU))
				.items(&MAIN_MENU),
			mirror_menu: Menu::new("Mirror", Some(icon::BORING)).items(&MIRROR_MENU),
			brush_radius: BRUSH_RADIUS,
			magnet: false,
			magnet_radius: MAGNET_RADIUS,
			history: History::default(),
			paste_pending: false,
			image_paste: false,
			request: None,
			pointer: None,
		}
	}
}

impl EditMode {
	pub fn update(&mut self, mesh: &mut Mesh, view: &View, response: &egui::Response) {
		let keyboard = !response.ctx.egui_wants_keyboard_input();
		let hovered = response.hovered();
		let action = response.ctx.input(|input| {
			self.pointer = input.pointer.latest_pos().or(self.pointer);
			let pointer = self.pointer.unwrap_or(response.rect.center());
			self.handle_input(mesh, view, input, view.to_world(pointer), hovered, keyboard)
		});
		self.handle_paste(mesh, view, response);
		if let Some((action, cursor)) = action {
			self.perform(mesh, view, &response.ctx, action, cursor);
		}
		self.settle_selection();
	}

	pub fn cancel(&mut self, mesh: &mut Mesh) {
		match std::mem::take(&mut self.operation) {
			Operation::Transform(transform) => {
				self.history.revert(mesh);
				self.selection = transform.before;
			}
			Operation::Bevel(bevel) => {
				self.history.revert(mesh);
				self.selection = bevel.before;
			}
			Operation::Palette { .. } => self.history.revert(mesh),
			_ => {}
		}
	}

	pub fn reset(&mut self, mesh: &Mesh) {
		self.operation = Operation::Idle;
		self.selection = Selection::default();
		self.history = History::new(mesh);
	}

	pub fn take_request(&mut self) -> Option<FileAction> {
		self.request.take()
	}

	pub fn take_image_paste(&mut self) -> bool {
		std::mem::take(&mut self.image_paste)
	}

	pub fn revision(&self) -> u64 {
		self.history.revision()
	}

	pub fn committed(&self) -> &Mesh {
		self.history.checkpoint()
	}

	pub fn move_layer(&mut self, mesh: &mut Mesh, layer: u32, target: usize, group: u32) {
		self.cancel(mesh);
		self.record(mesh, |_, mesh| mesh.move_layer(layer, target, group));
	}

	pub fn move_group(&mut self, mesh: &mut Mesh, group: u32, target: usize) {
		self.cancel(mesh);
		self.record(mesh, |_, mesh| mesh.move_group(group, target));
	}

	pub fn rename_layer(&mut self, mesh: &mut Mesh, layer: u32, name: String) {
		self.cancel(mesh);
		self.record(mesh, |_, mesh| mesh.rename_layer(layer, name));
	}

	pub fn rename_group(&mut self, mesh: &mut Mesh, group: u32, name: String) {
		self.cancel(mesh);
		self.record(mesh, |_, mesh| mesh.rename_group(group, name));
	}

	pub fn delete_layers(&mut self, mesh: &mut Mesh, layers: &[u32]) {
		self.cancel(mesh);
		self.record(mesh, |edit, mesh| {
			let removed: Vec<usize> = mesh.layer_vertices(layers).collect();
			edit.selection
				.vertices
				.retain(|vertex| !removed.contains(vertex));
			for vertex in &mut edit.selection.vertices {
				*vertex -= removed.iter().filter(|&removed| removed < vertex).count();
			}
			mesh.remove_vertices(removed);
		});
	}

	pub fn toggle_menu(&mut self, mesh: &mut Mesh, anchor: Pos2) {
		let open = self.menu_open();
		self.cancel(mesh);
		if !open {
			self.open_menu(anchor);
		}
	}

	pub fn toggle_mirror_menu(&mut self, mesh: &mut Mesh, group: u32, anchor: Pos2) {
		let open = self.mirror_menu_group() == Some(group);
		self.cancel(mesh);
		if !open {
			self.operation = Operation::MirrorMenu { anchor, group };
		}
	}

	pub fn mirror_menu_group(&self) -> Option<u32> {
		match self.operation {
			Operation::MirrorMenu { group, .. } => Some(group),
			_ => None,
		}
	}

	pub fn menu_open(&self) -> bool {
		matches!(self.operation, Operation::MainMenu { .. })
	}

	pub fn captures_scroll(&self) -> bool {
		matches!(
			self.operation,
			Operation::Transform(Transform {
				segments: Some(_),
				..
			}) | Operation::Transform(Transform {
				magnet: Some(_),
				..
			}) | Operation::Bevel(_)
				| Operation::Brush
		)
	}

	pub fn captures_middle(&self) -> bool {
		matches!(self.operation, Operation::Brush)
	}

	pub fn selection(&self) -> &[usize] {
		&self.selection.vertices
	}

	pub fn select_layers(&mut self, mesh: &mut Mesh, layers: &[u32], shift: bool) {
		self.cancel(mesh);
		self.operation = Operation::Idle;

		let within = |vertex: usize| layers.contains(&mesh.layer(vertex));
		let deselect = shift && self.selection.vertices.iter().any(|&vertex| within(vertex));
		if !shift {
			self.selection.vertices.clear();
		}

		self.selection.vertices.retain(|&vertex| !within(vertex));
		if !deselect {
			self.selection.vertices.extend(mesh.layer_vertices(layers));
		}
		self.settle_selection();
	}

	pub fn add_image(&mut self, mesh: &mut Mesh, image: Image) {
		self.cancel(mesh);
		self.record(mesh, |edit, mesh| {
			edit.selection = Selection {
				vertices: Vec::new(),
				images: vec![mesh.images.len()],
			};
			mesh.images.push(image);
		});
	}

	pub fn import(&mut self, mesh: &mut Mesh, shapes: &[Shape], center: Pos2) {
		let points: Vec<Pos2> = shapes
			.iter()
			.flat_map(|shape| shape.points.iter().copied())
			.collect();
		if points.is_empty() {
			return;
		}

		let offset = (center - Rect::from_points(&points).center()).round();
		self.cancel(mesh);
		self.record(mesh, |edit, mesh| {
			edit.selection = Selection {
				vertices: shapes
					.iter()
					.flat_map(|shape| mesh.add_shape(shape, offset))
					.collect(),
				images: Vec::new(),
			};
		});
	}

	pub fn show_menu(
		&mut self,
		ctx: &egui::Context,
		mesh: &mut Mesh,
		view: &View,
		accent: Color32,
	) {
		let pointer = ctx.pointer_latest_pos().unwrap_or_default();
		let (menu, anchor, pos) = match self.operation {
			Operation::Menu {
				pos,
				kind: MenuKind::Create,
			} => (&mut self.create_menu, view.to_screen(pos), pos),
			Operation::Menu {
				pos,
				kind: MenuKind::Merge,
			} => (&mut self.merge_menu, view.to_screen(pos), pos),
			Operation::MainMenu { anchor } => (&mut self.main_menu, anchor, view.to_world(pointer)),
			Operation::MirrorMenu { anchor, group } => {
				for (axis, active) in mesh.mirror(group).into_iter().enumerate() {
					self.mirror_menu.set_active(Action::Mirror(axis), active);
				}
				(&mut self.mirror_menu, anchor, view.to_world(pointer))
			}
			_ => return,
		};
		let Some(action) = menu.show(ctx, anchor, accent) else {
			return;
		};

		if let (Action::Mirror(axis), Some(group)) = (action, self.mirror_menu_group()) {
			self.record(mesh, |_, mesh| mesh.toggle_mirror(group, axis));
			return;
		}

		self.operation = Operation::Idle;
		self.perform(mesh, view, ctx, action, pos);
		self.settle_selection();
	}

	pub fn show_palette(
		&mut self,
		ctx: &egui::Context,
		mesh: &mut Mesh,
		view: &View,
		accent: Color32,
	) {
		let Operation::Palette { pos, color } = &mut self.operation else {
			return;
		};

		let mut changed = false;
		egui::Area::new(egui::Id::new("palette"))
			.order(egui::Order::Foreground)
			.fixed_pos(view.to_screen(*pos))
			.show(ctx, |ui| {
				panel::bordered_frame(accent).show(ui, |ui| {
					changed = color_picker::color_picker_color32(ui, color, Alpha::Opaque);
				});
			});

		if changed {
			mesh.set_color(&self.selection.vertices, *color);
		}
	}

	pub fn draw(
		&self,
		mesh: &Mesh,
		curve_outlines: &[(Rect, Vec<Pos2>)],
		view: &View,
		visible: Rect,
		painter: &egui::Painter,
		accent: Color32,
	) {
		let mut selected = vec![false; mesh.vertices.len()];
		for &index in &self.selection.vertices {
			selected[index] = true;
		}

		for &index in &self.selection.images {
			let points = mesh.images[index]
				.corners
				.map(|corner| view.to_screen(corner));
			painter.add(egui::Shape::closed_line(
				points.to_vec(),
				Stroke::new(EDGE_WIDTH, SELECTED_COLOR),
			));
		}

		let color = |is_selected: bool| if is_selected { SELECTED_COLOR } else { accent };

		if self.show_outlines {
			let curve_layers: HashSet<u32> = mesh
				.layers
				.iter()
				.filter(|layer| layer.curve)
				.map(|layer| layer.id)
				.collect();

			let mut shape = egui::Mesh::default();
			for &[a, b] in &mesh.edges {
				let (start, end) = (mesh.vertices[a], mesh.vertices[b]);
				if !Rect::from_two_pos(start, end).intersects(visible) {
					continue;
				}

				let color = color(selected[a] && selected[b]);
				let color = if curve_layers.contains(&mesh.layer(a)) {
					color.gamma_multiply(CAGE_OPACITY)
				} else {
					color
				};
				add_edge(
					&mut shape,
					view.to_screen(start),
					view.to_screen(end),
					color,
				);
			}
			painter.add(shape);

			for (_, outline) in curve_outlines
				.iter()
				.filter(|(bounds, _)| bounds.intersects(visible))
			{
				let points = outline.iter().map(|&pos| view.to_screen(pos)).collect();
				painter.add(egui::Shape::closed_line(
					points,
					Stroke::new(EDGE_WIDTH, accent),
				));
			}
		}

		if let Operation::Transform(transform) = &self.operation {
			if let Some(axis) = transform.axis
				&& let Some(direction) = transform.direction(axis)
			{
				axis_line(painter, axis, direction, view.to_screen(transform.pivot));
			}

			let [x, y] = transform.guides;
			if let Some(guide) = x {
				axis_line(painter, Axis::Y, Vec2::Y, view.to_screen(guide));
			}
			if let Some(guide) = y {
				axis_line(painter, Axis::X, Vec2::X, view.to_screen(guide));
			}
		}

		let radius = match &self.operation {
			Operation::Brush => Some(self.brush_radius),
			Operation::Transform(Transform {
				magnet: Some(_), ..
			}) => Some(self.magnet_radius),
			_ => None,
		};
		if let Some(radius) = radius
			&& let Some(pos) = painter.ctx().pointer_hover_pos()
		{
			painter.circle_stroke(pos, radius, Stroke::new(TOOL_WIDTH, TOOL_COLOR));
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

		if !self.show_outlines {
			return;
		}

		let mut shape = egui::Mesh::default();
		for (index, &vertex) in mesh.vertices.iter().enumerate() {
			if !visible.contains(vertex) {
				continue;
			}

			let center = view.to_screen(vertex);
			let color = color(selected[index]);
			if mesh.sharp[index] {
				add_diamond(&mut shape, center, color);
			} else {
				shape.add_colored_rect(
					Rect::from_center_size(center, Vec2::splat(VERTEX_SIZE)),
					color,
				);
			}
		}
		painter.add(shape);
	}

	fn handle_input(
		&mut self,
		mesh: &mut Mesh,
		view: &View,
		input: &egui::InputState,
		cursor: Pos2,
		hovered: bool,
		keyboard: bool,
	) -> Option<(Action, Pos2)> {
		let key = |key: Key| keyboard && input.key_pressed(key);
		let pressed = |button: PointerButton| input.pointer.button_pressed(button);

		match &mut self.operation {
			Operation::Idle => {
				if keyboard && let Some(action) = key_action(input) {
					return Some((action, cursor));
				} else if hovered && pressed(PointerButton::Secondary) {
					let radius = VERTEX_HIT_RADIUS / view.scale;
					let outlines = self.show_outlines;
					let hit = mesh.nearest_vertex(cursor, radius).filter(|_| outlines);
					if hit.is_none()
						&& outlines && let Some(face) = mesh.face_at(cursor)
					{
						self.select_face(face, input.modifiers.shift);
					} else if hit.is_none()
						&& self.show_images
						&& let Some(image) = mesh.image_at(cursor)
					{
						self.select_image(image, input.modifiers.shift);
					} else {
						self.select(hit, input.modifiers.shift);
					}
				}
			}
			Operation::Grab { isolate } => {
				if !input.pointer.button_down(PointerButton::Secondary) {
					if let Some(isolate) = isolate.take() {
						self.selection = isolate;
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
						self.selection.vertices.clone(),
					);
					self.attach_magnet(mesh);
				}
			}
			Operation::Transform(transform) => {
				if key(Key::X) {
					transform.toggle_axis(Axis::X);
				} else if key(Key::Y) {
					transform.toggle_axis(Axis::Y);
				} else if key(Key::N) {
					transform.toggle_axis(Axis::Normal);
				} else if key(Key::Backspace) {
					transform.typed.pop();
				}

				if keyboard && transform.kind == TransformKind::Rotate {
					for event in &input.events {
						if let Event::Text(text) = event {
							text.chars().for_each(|symbol| transform.type_char(symbol));
						}
					}
				}

				let steps = wheel_steps(input);
				if transform.primitive && input.modifiers.shift && steps != 0 {
					let factor = PRIMITIVE_SCALE_STEP.powi(steps as i32);
					for pos in &mut transform.original {
						*pos = transform.pivot + (*pos - transform.pivot) * factor;
					}
				} else if transform.magnet.is_some() && steps != 0 {
					self.magnet_radius = (self.magnet_radius + steps as f32 * BRUSH_STEP)
						.clamp(MIN_BRUSH_RADIUS, MAX_BRUSH_RADIUS);
				} else if let Some(segments) = &mut transform.segments
					&& steps != 0
				{
					*segments = segments
						.saturating_add_signed(steps)
						.clamp(MIN_CIRCLE_SEGMENTS, MAX_CIRCLE_SEGMENTS);
					let radius = transform.original[0].distance(transform.pivot);
					let curve = mesh.is_curve(self.selection.vertices[0]);
					mesh.remove_vertices(std::mem::take(&mut self.selection.vertices));
					self.selection.vertices =
						mesh.add_loop(&circle_points(transform.pivot, *segments, radius), curve);
					transform.original = self
						.selection
						.vertices
						.iter()
						.map(|&index| mesh.vertices[index])
						.collect();
				}

				transform_selection(
					mesh,
					&self.selection,
					transform,
					cursor,
					input.modifiers.alt,
				);

				transform.guides = if input.modifiers.shift
					&& !transform.primitive
					&& transform.kind == TransformKind::Translate
					&& transform.axis != Some(Axis::Normal)
				{
					align(
						mesh,
						&self.selection.vertices,
						transform.free(),
						ALIGN_RADIUS / view.scale,
					)
				} else {
					[None; 2]
				};

				attract(
					mesh,
					&self.selection.vertices,
					transform,
					self.magnet_radius / view.scale,
				);

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
			Operation::Bevel(bevel) => {
				let steps = wheel_steps(input);
				if steps != 0 {
					bevel.segments = bevel
						.segments
						.saturating_add_signed(steps)
						.clamp(MIN_BEVEL_SEGMENTS, MAX_BEVEL_SEGMENTS);
					self.history.revert(mesh);
					bevel.corners = mesh.bevel(&bevel.before.vertices, bevel.segments);
					self.selection = bevel.selection();
				}
				bevel.apply(mesh, cursor);

				if key(Key::Escape) || pressed(PointerButton::Secondary) {
					self.cancel(mesh);
				} else if key(Key::Enter) || pressed(PointerButton::Primary) {
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
					self.selection
						.vertices
						.retain(|vertex| !inside.contains(vertex));
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
			Operation::Menu { .. } | Operation::MainMenu { .. } | Operation::MirrorMenu { .. } => {
				if key(Key::Escape) || (hovered && input.pointer.any_pressed()) {
					self.operation = Operation::Idle;
				}
			}
			Operation::Palette { .. } => {
				if key(Key::Escape) {
					self.cancel(mesh);
				} else if key(Key::Enter) || (hovered && input.pointer.any_pressed()) {
					self.confirm(mesh);
				}
			}
		}

		None
	}

	fn handle_paste(&mut self, mesh: &mut Mesh, view: &View, response: &egui::Response) {
		let ctx = &response.ctx;
		let pending = std::mem::replace(&mut self.paste_pending, false);
		if !matches!(self.operation, Operation::Idle) || ctx.egui_wants_keyboard_input() {
			return;
		}

		self.image_paste = ctx.input(|input| input.modifiers.command && input.key_released(Key::V));
		let pasted = ctx.input(|input| {
			input.events.iter().find_map(|event| match event {
				Event::Paste(text) => Some(text.clone()),
				_ => None,
			})
		});
		let Some(text) = pasted else {
			return;
		};

		let shapes = import::parse(&text);
		if !shapes.is_empty() {
			let pointer = ctx.pointer_latest_pos().unwrap_or(response.rect.center());
			self.import(mesh, &shapes, view.to_world(pointer));
		} else if pending && let Some(color) = parse_color(&text) {
			self.record(mesh, |edit, mesh| {
				mesh.set_color(&edit.selection.vertices, color)
			});
		}
	}

	fn open_menu(&mut self, anchor: Pos2) {
		self.main_menu.reset();
		self.operation = Operation::MainMenu { anchor };
	}

	fn perform(
		&mut self,
		mesh: &mut Mesh,
		view: &View,
		ctx: &egui::Context,
		action: Action,
		cursor: Pos2,
	) {
		match action {
			Action::File(action) => self.request = Some(action),
			Action::Undo => self.history.undo(mesh, &mut self.selection),
			Action::Redo => self.history.redo(mesh, &mut self.selection),
			Action::Translate => {
				self.begin_transform(
					mesh,
					TransformKind::Translate,
					cursor,
					false,
					self.selection.vertices.clone(),
				);
				self.attach_magnet(mesh);
			}
			Action::Rotate => self.begin_transform(
				mesh,
				TransformKind::Rotate,
				cursor,
				false,
				self.selection.vertices.clone(),
			),
			Action::Scale => self.begin_transform(
				mesh,
				TransformKind::Scale,
				cursor,
				false,
				self.selection.vertices.clone(),
			),
			Action::Subdivide | Action::SubdivideCurve => {
				let smooth = matches!(action, Action::SubdivideCurve);
				self.record(mesh, |edit, mesh| {
					let midpoints = mesh.subdivide(&edit.selection.vertices, smooth);
					edit.extend_selection(midpoints);
				});
			}
			Action::Decimate => self.record(mesh, |edit, mesh| {
				edit.selection.vertices = mesh.decimate(&edit.selection.vertices);
			}),
			Action::Space => self.record(mesh, |edit, mesh| mesh.space(&edit.selection.vertices)),
			Action::Extrude => self.create(mesh, cursor, Mesh::extrude),
			Action::Duplicate => self.create(mesh, cursor, Mesh::duplicate),
			Action::Inset => self.inset(mesh, cursor),
			Action::Bevel => self.bevel(mesh, cursor),
			Action::Connect => {
				if let [a, b] = self.selection.vertices[..] {
					self.record(mesh, |_, mesh| mesh.add_edge(a, b));
				}
			}
			Action::AddVertex => self.record(mesh, |edit, mesh| {
				edit.selection.vertices = vec![mesh.add_vertex(cursor)]
			}),
			Action::AddRect => self.add_primitive(mesh, &rect_points(cursor), cursor, None, false),
			Action::AddCircle => self.add_primitive(
				mesh,
				&circle_points(cursor, CIRCLE_SEGMENTS, CIRCLE_RADIUS),
				cursor,
				Some(CIRCLE_SEGMENTS),
				false,
			),
			Action::AddCurve => self.add_primitive(
				mesh,
				&circle_points(cursor, CURVE_POINTS, CIRCLE_RADIUS),
				cursor,
				Some(CURVE_POINTS),
				true,
			),
			Action::SelectAll => {
				let empty = self.selection.vertices.is_empty() && self.selection.images.is_empty();
				self.selection = Selection::default();
				if empty {
					self.selection.vertices = (0..mesh.vertices.len()).collect();
				}
			}
			Action::SelectLinked => {
				let radius = LINK_PICK_RADIUS / view.scale;
				let seeds = match mesh.nearest_vertex(cursor, radius) {
					Some(vertex) => vec![vertex],
					None => self.selection.vertices.clone(),
				};
				self.extend_selection(mesh.linked(&seeds));
			}
			Action::SelectSameEdge => {
				self.extend_selection(mesh.edge_loops(&self.selection.vertices));
			}
			Action::Brush => self.operation = Operation::Brush,
			Action::BoxSelect => self.operation = Operation::BoxSelect { start: None },
			Action::Magnet => self.magnet = !self.magnet,
			Action::ToggleOutlines => self.show_outlines = !self.show_outlines,
			Action::ToggleImages => self.show_images = !self.show_images,
			Action::Menu(kind) => self.operation = Operation::Menu { pos: cursor, kind },
			Action::MainMenu => self.open_menu(view.to_screen(cursor)),
			Action::Merge(target) => {
				self.record(mesh, |edit, mesh| edit.merge(mesh, target, cursor));
			}
			Action::Dissolve => self.record(mesh, |edit, mesh| {
				mesh.dissolve(std::mem::take(&mut edit.selection.vertices));
			}),
			Action::ToggleHole => self.record(mesh, |edit, mesh| {
				mesh.toggle_hole(&edit.selection.vertices)
			}),
			Action::ToggleHoldout => self.record(mesh, |edit, mesh| {
				mesh.toggle_holdout(&edit.selection.vertices)
			}),
			Action::ToggleSharp => self.record(mesh, |edit, mesh| {
				mesh.toggle_sharp(&edit.selection.vertices)
			}),
			Action::Group => self.record(mesh, |edit, mesh| mesh.group(&edit.selection.vertices)),
			Action::Ungroup => {
				self.record(mesh, |edit, mesh| mesh.ungroup(&edit.selection.vertices))
			}
			Action::Mirror(_) => {}
			Action::Palette => {
				if let Some(color) = mesh.face_color(&self.selection.vertices) {
					self.operation = Operation::Palette { pos: cursor, color };
				}
			}
			Action::CopyColor => {
				if let Some(color) = mesh.face_color(&self.selection.vertices) {
					ctx.copy_text(HexColor::Hex6(color).to_string());
				}
			}
			Action::PasteColor => {
				self.paste_pending = true;
				ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
				ctx.request_repaint();
			}
			Action::CopySvg => {
				if let Some(svg) = export::svg(mesh.fills(&self.selection.vertices)) {
					ctx.copy_text(svg);
				}
			}
			Action::Delete => self.record(mesh, |edit, mesh| {
				mesh.remove_vertices(std::mem::take(&mut edit.selection.vertices));
				mesh.remove_images(std::mem::take(&mut edit.selection.images));
			}),
		}
	}

	fn confirm(&mut self, mesh: &Mesh) {
		let before = match std::mem::take(&mut self.operation) {
			Operation::Transform(transform) => transform.before,
			Operation::Bevel(bevel) => bevel.before,
			Operation::Palette { .. } => self.selection.clone(),
			_ => return,
		};

		self.history.commit(mesh, before, &self.selection);
	}

	fn record(&mut self, mesh: &mut Mesh, action: impl FnOnce(&mut Self, &mut Mesh)) {
		let before = self.selection.clone();
		action(self, mesh);
		mesh.prune_groups();
		self.history.commit(mesh, before, &self.selection);
	}

	fn select(&mut self, hit: Option<usize>, shift: bool) {
		let Some(index) = hit else {
			if !shift {
				self.selection = Selection::default();
			}
			return;
		};

		let position = self
			.selection
			.vertices
			.iter()
			.position(|&selected| selected == index);
		match (position, shift) {
			(Some(position), true) => {
				self.selection.vertices.remove(position);
				return;
			}
			(None, true) => self.selection.vertices.push(index),
			(None, false) => self.selection.vertices = vec![index],
			(Some(_), false) => {}
		}

		let isolate = (!shift && self.selection.vertices.len() > 1).then(|| Selection {
			vertices: vec![index],
			images: Vec::new(),
		});
		self.operation = Operation::Grab { isolate };
	}

	fn select_image(&mut self, index: usize, shift: bool) {
		self.selection.vertices.clear();
		let images = &mut self.selection.images;
		match (images.iter().position(|&image| image == index), shift) {
			(Some(position), true) => {
				images.remove(position);
				return;
			}
			(None, true) => images.push(index),
			(None, false) => *images = vec![index],
			(Some(_), false) => {}
		}

		let isolate = (!shift && images.len() > 1).then(|| Selection {
			vertices: Vec::new(),
			images: vec![index],
		});
		self.operation = Operation::Grab { isolate };
	}

	fn settle_selection(&mut self) {
		let editing = matches!(
			self.operation,
			Operation::Transform(_) | Operation::Bevel(_) | Operation::Palette { .. }
		);
		if !self.show_outlines && !editing {
			self.selection.vertices.clear();
		}
		if !self.show_images {
			self.selection.images.clear();
		}
		if !self.selection.vertices.is_empty() {
			self.selection.images.clear();
		}
	}

	fn select_face(&mut self, face: Vec<usize>, shift: bool) {
		if shift
			&& face
				.iter()
				.all(|vertex| self.selection.vertices.contains(vertex))
		{
			self.selection
				.vertices
				.retain(|vertex| !face.contains(vertex));
			return;
		}

		if !shift {
			self.selection.vertices.clear();
		}

		self.extend_selection(face);
		self.operation = Operation::Grab { isolate: None };
	}

	fn extend_selection(&mut self, vertices: impl IntoIterator<Item = usize>) {
		for vertex in vertices {
			if !self.selection.vertices.contains(&vertex) {
				self.selection.vertices.push(vertex);
			}
		}
	}

	fn create(
		&mut self,
		mesh: &mut Mesh,
		cursor: Pos2,
		create: fn(&mut Mesh, &[usize]) -> Vec<usize>,
	) {
		if self.selection.vertices.is_empty() {
			return;
		}

		let sources = std::mem::take(&mut self.selection.vertices);
		self.selection.vertices = create(mesh, &sources);
		self.begin_transform(mesh, TransformKind::Translate, cursor, false, sources);
	}

	fn inset(&mut self, mesh: &mut Mesh, cursor: Pos2) {
		let (inner, directions) = mesh.inset(&self.selection.vertices);
		if inner.is_empty() {
			return;
		}

		let sources = std::mem::replace(&mut self.selection.vertices, inner);
		self.begin_transform(
			mesh,
			TransformKind::Inset(directions),
			cursor,
			false,
			sources,
		);
	}

	fn bevel(&mut self, mesh: &mut Mesh, cursor: Pos2) {
		let corners = mesh.bevel(&self.selection.vertices, MIN_BEVEL_SEGMENTS);
		if corners.is_empty() {
			return;
		}

		let bevel = Bevel {
			anchor: cursor,
			pivot: center(corners.iter().map(|corner| corner.pos)),
			corners,
			segments: MIN_BEVEL_SEGMENTS,
			before: self.selection.clone(),
		};
		self.selection = bevel.selection();
		self.operation = Operation::Bevel(bevel);
	}

	fn add_primitive(
		&mut self,
		mesh: &mut Mesh,
		points: &[Pos2],
		cursor: Pos2,
		segments: Option<usize>,
		curve: bool,
	) {
		let sources = std::mem::replace(&mut self.selection.vertices, mesh.add_loop(points, curve));
		self.begin_transform(mesh, TransformKind::Translate, cursor, false, sources);
		if let Operation::Transform(transform) = &mut self.operation {
			transform.primitive = true;
			transform.segments = segments;
		}
	}

	fn merge(&mut self, mesh: &mut Mesh, target: MergeTarget, cursor: Pos2) {
		let (Some(&first), Some(&last)) = (
			self.selection.vertices.first(),
			self.selection.vertices.last(),
		) else {
			return;
		};

		let pos = match target {
			MergeTarget::Last => mesh.vertices[last],
			MergeTarget::Center => center(
				self.selection
					.vertices
					.iter()
					.map(|&index| mesh.vertices[index]),
			),
			MergeTarget::First => mesh.vertices[first],
			MergeTarget::Cursor => cursor,
		};
		self.selection.vertices = vec![mesh.merge(&self.selection.vertices, pos)];
	}

	fn begin_transform(
		&mut self,
		mesh: &Mesh,
		kind: TransformKind,
		anchor: Pos2,
		drag: bool,
		before: Vec<usize>,
	) {
		self.settle_selection();
		let vertices = self
			.selection
			.vertices
			.iter()
			.map(|&index| mesh.vertices[index]);
		let corners = self
			.selection
			.images
			.iter()
			.flat_map(|&index| mesh.images[index].corners);
		let original: Vec<Pos2> = vertices.chain(corners).collect();
		if original.is_empty() {
			return;
		}

		let normal = mesh.normal(&before);
		self.operation = Operation::Transform(Transform {
			kind,
			anchor,
			pivot: center(original.iter().copied()),
			original,
			drag,
			before: Selection {
				vertices: before,
				images: self.selection.images.clone(),
			},
			axis: None,
			normal,
			primitive: false,
			segments: None,
			typed: String::new(),
			guides: [None; 2],
			magnet: None,
		});
	}

	fn attach_magnet(&mut self, mesh: &Mesh) {
		let Operation::Transform(transform) = &mut self.operation else {
			return;
		};
		if !self.magnet || self.selection.vertices.is_empty() {
			return;
		}

		let rest = mesh
			.vertices
			.iter()
			.enumerate()
			.filter(|(index, _)| !self.selection.vertices.contains(index))
			.map(|(index, &pos)| (index, pos))
			.collect();
		transform.magnet = Some(rest);
	}
}

fn key_action(input: &egui::InputState) -> Option<Action> {
	let key = |key: Key| input.key_pressed(key);
	let modifiers = input.modifiers;
	let plain = |code: Key| key(code) && modifiers.is_none();
	let action = if input.events.contains(&Event::Copy) {
		Action::CopySvg
	} else if key(Key::Z) && modifiers.ctrl {
		Action::Undo
	} else if key(Key::R) && modifiers.ctrl {
		Action::Redo
	} else if key(Key::G) && modifiers.ctrl && modifiers.shift {
		Action::Ungroup
	} else if key(Key::G) && modifiers.ctrl {
		Action::Group
	} else if plain(Key::G) {
		Action::Translate
	} else if plain(Key::R) {
		Action::Rotate
	} else if key(Key::S) && modifiers.alt {
		Action::SubdivideCurve
	} else if key(Key::S) && modifiers.ctrl {
		Action::File(FileAction::Save)
	} else if key(Key::S) && modifiers.shift {
		Action::Subdivide
	} else if plain(Key::S) {
		Action::Scale
	} else if plain(Key::F) {
		Action::Connect
	} else if key(Key::D) && modifiers.shift {
		Action::Duplicate
	} else if plain(Key::D) {
		Action::Decimate
	} else if plain(Key::E) {
		Action::Extrude
	} else if plain(Key::I) {
		Action::Inset
	} else if plain(Key::V) {
		Action::AddVertex
	} else if plain(Key::A) {
		Action::SelectAll
	} else if plain(Key::L) {
		Action::SelectLinked
	} else if plain(Key::T) {
		Action::SelectSameEdge
	} else if plain(Key::Q) {
		Action::MainMenu
	} else if plain(Key::W) {
		Action::Menu(MenuKind::Create)
	} else if plain(Key::M) {
		Action::Menu(MenuKind::Merge)
	} else if plain(Key::X) {
		Action::Dissolve
	} else if plain(Key::N) {
		Action::Space
	} else if plain(Key::H) {
		Action::ToggleHoldout
	} else if plain(Key::U) {
		Action::ToggleSharp
	} else if plain(Key::P) {
		Action::ToggleHole
	} else if key(Key::Y) && modifiers.ctrl {
		Action::CopyColor
	} else if key(Key::Y) && modifiers.shift {
		Action::PasteColor
	} else if plain(Key::Y) {
		Action::Palette
	} else if plain(Key::C) {
		Action::Brush
	} else if key(Key::B) && modifiers.ctrl {
		Action::Bevel
	} else if plain(Key::B) {
		Action::BoxSelect
	} else if plain(Key::K) {
		Action::Magnet
	} else if key(Key::Z) && modifiers.shift {
		Action::ToggleImages
	} else if plain(Key::Z) {
		Action::ToggleOutlines
	} else if plain(Key::Delete) {
		Action::Delete
	} else {
		return None;
	};
	Some(action)
}

fn center(points: impl ExactSizeIterator<Item = Pos2>) -> Pos2 {
	let count = points.len() as f32;
	let sum = points.fold(Vec2::ZERO, |sum, pos| sum + pos.to_vec2());
	(sum / count).to_pos2()
}

fn transform_selection(
	mesh: &mut Mesh,
	selection: &Selection,
	transform: &Transform,
	cursor: Pos2,
	snap: bool,
) {
	let count = selection.vertices.len();
	let mut positions = transform
		.original
		.iter()
		.enumerate()
		.map(|(index, &pos)| transform.apply(index, pos, cursor, snap && index < count));

	for (&vertex, pos) in selection.vertices.iter().zip(&mut positions) {
		mesh.vertices[vertex] = pos;
	}

	for &image in &selection.images {
		let corners = &mut mesh.images[image].corners;
		for (corner, pos) in corners.iter_mut().zip(&mut positions) {
			*corner = pos;
		}

		if snap && transform.kind == TransformKind::Translate {
			let offset = transform.constrain(corners[0].round() - corners[0]);
			corners.iter_mut().for_each(|corner| *corner += offset);
		}
	}
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

fn attract(mesh: &mut Mesh, selection: &[usize], transform: &Transform, radius: f32) {
	let Some(rest) = &transform.magnet else {
		return;
	};

	for &(vertex, pos) in rest {
		let nearest = selection
			.iter()
			.zip(&transform.original)
			.map(|(&selected, &origin)| (origin.distance(pos), mesh.vertices[selected] - origin))
			.min_by(|a, b| a.0.total_cmp(&b.0));
		let offset = match nearest {
			Some((distance, offset)) if distance < radius => {
				let weight = 1.0 - distance / radius;
				offset * weight * weight * (3.0 - 2.0 * weight)
			}
			_ => Vec2::ZERO,
		};
		mesh.vertices[vertex] = pos + offset;
	}
}

fn axis_line(painter: &egui::Painter, axis: Axis, direction: Vec2, pos: Pos2) {
	let color = match axis {
		Axis::X => AXIS_X_COLOR,
		Axis::Y => AXIS_Y_COLOR,
		Axis::Normal => AXIS_NORMAL_COLOR,
	};
	let clip = painter.clip_rect();
	let reach = direction * (clip.size().length() + clip.center().distance(pos));
	painter.line_segment([pos - reach, pos + reach], Stroke::new(AXIS_WIDTH, color));
}

fn add_edge(shape: &mut egui::Mesh, a: Pos2, b: Pos2, color: Color32) {
	let offset = (b - a).normalized().rot90() * (EDGE_WIDTH / 2.0);
	let index = shape.vertices.len() as u32;
	shape.colored_vertex(a + offset, color);
	shape.colored_vertex(b + offset, color);
	shape.colored_vertex(b - offset, color);
	shape.colored_vertex(a - offset, color);
	shape.add_triangle(index, index + 1, index + 2);
	shape.add_triangle(index, index + 2, index + 3);
}

fn add_diamond(shape: &mut egui::Mesh, center: Pos2, color: Color32) {
	let (x, y) = (Vec2::X * (SHARP_SIZE / 2.0), Vec2::Y * (SHARP_SIZE / 2.0));
	let index = shape.vertices.len() as u32;
	shape.colored_vertex(center - y, color);
	shape.colored_vertex(center + x, color);
	shape.colored_vertex(center + y, color);
	shape.colored_vertex(center - x, color);
	shape.add_triangle(index, index + 1, index + 2);
	shape.add_triangle(index, index + 2, index + 3);
}

fn parse_color(text: &str) -> Option<Color32> {
	let text = text.trim();
	if !text.starts_with('#') || ![4, 7].contains(&text.len()) {
		return None;
	}

	Color32::from_hex(text).ok()
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
