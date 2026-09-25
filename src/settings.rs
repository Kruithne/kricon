use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

const FILE_NAME: &str = "kricon.ini";
const DEFAULT_FACE_OPACITY: f32 = 0.1;

#[derive(Clone, PartialEq)]
pub struct Settings {
	pub workspace: Option<PathBuf>,
	pub face_opacity: f32,
	pub spacing: f32,
	pub show_images: bool,
	pub show_outlines: bool,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			workspace: None,
			face_opacity: DEFAULT_FACE_OPACITY,
			spacing: 0.0,
			show_images: true,
			show_outlines: true,
		}
	}
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
				"face_opacity" => parse(value, &mut settings.face_opacity),
				"spacing" => parse(value, &mut settings.spacing),
				"show_images" => parse(value, &mut settings.show_images),
				"show_outlines" => parse(value, &mut settings.show_outlines),
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
		let text = format!(
			"workspace={workspace}\nface_opacity={}\nspacing={}\nshow_images={}\nshow_outlines={}\n",
			self.face_opacity, self.spacing, self.show_images, self.show_outlines
		);
		if let Some(path) = path() {
			let _ = fs::write(path, text);
		}
	}
}

fn parse<T: FromStr>(value: &str, target: &mut T) {
	if let Ok(value) = value.parse() {
		*target = value;
	}
}

fn path() -> Option<PathBuf> {
	Some(std::env::current_exe().ok()?.with_file_name(FILE_NAME))
}
