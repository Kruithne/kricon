use crate::panel::{self, Header};
use eframe::egui;

const ITEM_WIDTH: f32 = 160.0;
const ARROW_SIZE: f32 = 4.0;

pub struct Menu<T> {
	id: egui::Id,
	header: Header,
	items: Vec<Item<T>>,
	open: Option<usize>,
}

struct Item<T> {
	entry: Entry<T>,
	label: &'static str,
	active: bool,
}

enum Entry<T> {
	Action(T),
	Submenu(Menu<T>),
}

impl<T: Copy + PartialEq> Menu<T> {
	pub fn new(title: &'static str, icon: Option<&str>) -> Self {
		Self {
			id: egui::Id::new("menu").with(title),
			header: Header::new(title, icon),
			items: Vec::new(),
			open: None,
		}
	}

	pub fn items(mut self, items: &[(T, &'static str)]) -> Self {
		for &(action, label) in items {
			self.push(Entry::Action(action), label);
		}
		self
	}

	pub fn submenu(mut self, label: &'static str, menu: Menu<T>) -> Self {
		self.push(Entry::Submenu(menu), label);
		self
	}

	pub fn set_active(&mut self, action: T, active: bool) {
		for item in &mut self.items {
			if let Entry::Action(other) = item.entry
				&& other == action
			{
				item.active = active;
			}
		}
	}

	pub fn reset(&mut self) {
		self.open = None;
	}

	pub fn show(
		&mut self,
		ctx: &egui::Context,
		pos: egui::Pos2,
		accent: egui::Color32,
	) -> Option<T> {
		let mut chosen = None;
		let mut submenu_pos = None;

		egui::Area::new(self.id)
			.order(egui::Order::Foreground)
			.fixed_pos(pos)
			.show(ctx, |ui| {
				panel::bordered_frame(accent).show(ui, |ui| {
					ui.spacing_mut().item_spacing.y = 2.0;
					let width = self
						.items
						.iter()
						.map(|item| match item.entry {
							Entry::Action(_) => panel::row_width(ui, item.label),
							Entry::Submenu(_) => {
								panel::row_width(ui, item.label) + ARROW_SIZE + panel::PADDING
							}
						})
						.fold(ITEM_WIDTH, f32::max);
					self.header.show(ui, width, accent);

					for (index, item) in self.items.iter_mut().enumerate() {
						let (rect, response, color) = panel::item(
							ui,
							egui::vec2(width, panel::ITEM_HEIGHT),
							self.open == Some(index) || item.active,
							egui::Sense::click(),
						);
						if response.hovered() {
							self.open = matches!(item.entry, Entry::Submenu(_)).then_some(index);
						}

						match item.entry {
							Entry::Action(action) if response.clicked() => chosen = Some(action),
							Entry::Submenu(_) => {
								if self.open == Some(index) {
									submenu_pos = Some(
										rect.right_top()
											+ egui::vec2(
												2.0 * panel::FRAME_MARGIN,
												-panel::FRAME_MARGIN,
											),
									);
								}
								paint_arrow(ui, rect, color);
							}
							Entry::Action(_) => {}
						}

						panel::paint_label(ui, panel::lead_rect(rect), item.label, color);
					}
				});
			});

		if let (Some(index), Some(pos)) = (self.open, submenu_pos)
			&& let Entry::Submenu(menu) = &mut self.items[index].entry
		{
			chosen = chosen.or(menu.show(ctx, pos, accent));
		}

		chosen
	}

	fn push(&mut self, entry: Entry<T>, label: &'static str) {
		self.items.push(Item {
			entry,
			label,
			active: false,
		});
	}
}

fn paint_arrow(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32) {
	let tip = rect.right_center() - egui::vec2(panel::PADDING, 0.0);
	ui.painter().add(egui::Shape::convex_polygon(
		vec![
			tip,
			tip + egui::vec2(-ARROW_SIZE, ARROW_SIZE),
			tip + egui::vec2(-ARROW_SIZE, -ARROW_SIZE),
		],
		color,
		egui::Stroke::NONE,
	));
}
