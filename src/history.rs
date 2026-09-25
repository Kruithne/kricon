use crate::edit::Selection;
use crate::mesh::{Change, Mesh};
use std::collections::VecDeque;

const MAX_ENTRIES: usize = 100;

pub struct Splice<T> {
	start: usize,
	before: Vec<T>,
	after: Vec<T>,
}

impl<T: Clone + PartialEq> Splice<T> {
	pub fn new(before: &[T], after: &[T]) -> Self {
		let start = before.iter().zip(after).take_while(|(a, b)| a == b).count();
		let end = before[start..]
			.iter()
			.rev()
			.zip(after[start..].iter().rev())
			.take_while(|(a, b)| a == b)
			.count();

		Self {
			start,
			before: before[start..before.len() - end].to_vec(),
			after: after[start..after.len() - end].to_vec(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.before.is_empty() && self.after.is_empty()
	}

	pub fn apply(&self, list: &mut Vec<T>, forward: bool) {
		let (from, to) = if forward {
			(&self.before, &self.after)
		} else {
			(&self.after, &self.before)
		};
		list.splice(self.start..self.start + from.len(), to.iter().cloned());
	}
}

struct Entry {
	id: u64,
	change: Change,
	before: Selection,
	after: Selection,
}

#[derive(Default)]
pub struct History {
	checkpoint: Mesh,
	undo: VecDeque<Entry>,
	redo: Vec<Entry>,
	next_id: u64,
	base: u64,
}

impl History {
	pub fn new(mesh: &Mesh) -> Self {
		Self {
			checkpoint: mesh.clone(),
			..Default::default()
		}
	}

	pub fn checkpoint(&self) -> &Mesh {
		&self.checkpoint
	}

	pub fn revision(&self) -> u64 {
		self.undo.back().map_or(self.base, |entry| entry.id)
	}

	pub fn commit(&mut self, mesh: &Mesh, before: Selection, after: &Selection) {
		let change = self.checkpoint.diff(mesh);
		if change.is_empty() {
			return;
		}

		self.checkpoint.apply(&change, true);
		if self.undo.len() == MAX_ENTRIES
			&& let Some(entry) = self.undo.pop_front()
		{
			self.base = entry.id;
		}

		self.next_id += 1;
		self.undo.push_back(Entry {
			id: self.next_id,
			change,
			before,
			after: after.clone(),
		});
		self.redo.clear();
	}

	pub fn revert(&self, mesh: &mut Mesh) {
		mesh.clone_from(&self.checkpoint);
	}

	pub fn undo(&mut self, mesh: &mut Mesh, selection: &mut Selection) {
		let Some(entry) = self.undo.pop_back() else {
			return;
		};

		mesh.apply(&entry.change, false);
		self.checkpoint.apply(&entry.change, false);
		selection.clone_from(&entry.before);
		self.redo.push(entry);
	}

	pub fn redo(&mut self, mesh: &mut Mesh, selection: &mut Selection) {
		let Some(entry) = self.redo.pop() else {
			return;
		};

		mesh.apply(&entry.change, true);
		self.checkpoint.apply(&entry.change, true);
		selection.clone_from(&entry.after);
		self.undo.push_back(entry);
	}
}
