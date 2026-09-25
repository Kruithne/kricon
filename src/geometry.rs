use eframe::egui::{Pos2, Vec2};
use std::f32::consts::TAU;

pub fn cross(a: Vec2, b: Vec2) -> f32 {
	a.x * b.y - a.y * b.x
}

pub fn turn(from: Vec2, to: Vec2) -> f32 {
	cross(from, to).atan2(from.dot(to)).rem_euclid(TAU)
}

pub fn area(polygon: &[Pos2]) -> f32 {
	let mut area = 0.0;
	for (index, &a) in polygon.iter().enumerate() {
		let b = polygon[(index + 1) % polygon.len()];
		area += cross(a.to_vec2(), b.to_vec2());
	}
	area / 2.0
}

pub fn encloses(polygon: &[Pos2], pos: Pos2) -> bool {
	let mut inside = false;
	for (index, &a) in polygon.iter().enumerate() {
		let b = polygon[(index + 1) % polygon.len()];
		if (a.y > pos.y) != (b.y > pos.y) && pos.x < a.x + (pos.y - a.y) * (b.x - a.x) / (b.y - a.y)
		{
			inside = !inside;
		}
	}
	inside
}
