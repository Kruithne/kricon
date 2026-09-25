use std::fs;
use std::path::PathBuf;

const FILE_NAME: &str = "kricon.ini";

#[derive(Default, Clone, PartialEq)]
pub struct Settings {
	pub workspace: Option<PathBuf>,
}

impl Settings {
	pub fn load() -> Self {
		let mut settings = Self::default();
		let Some(text) = path().and_then(|path| fs::read_to_string(path).ok()) else {
			return settings;
		};

		for (key, value) in text.lines().filter_map(|line| line.split_once('=')) {
			let value = value.trim();
			match key.trim() {
				"workspace" if !value.is_empty() => settings.workspace = Some(value.into()),
				_ => {}
			}
		}
		settings
	}

	pub fn save(&self) {
		let workspace = self
			.workspace
			.as_deref()
			.map(|path| path.to_string_lossy())
			.unwrap_or_default();
		if let Some(path) = path() {
			let _ = fs::write(path, format!("workspace={workspace}\n"));
		}
	}
}

fn path() -> Option<PathBuf> {
	Some(std::env::current_exe().ok()?.with_file_name(FILE_NAME))
}
