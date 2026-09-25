use crate::images::{self, Decoded};
use crate::mesh::{Layer, Mesh};
use eframe::egui::{self, Color32, Pos2, Vec2, pos2, vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};
use std::{fs, io, thread};

pub const EXTENSION: &str = "kri";
const MAGIC: &[u8; 4] = b"KRI\0";
const VERSION: u32 = 1;
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(600);
const META: &[u8; 4] = b"META";
const VERTICES: &[u8; 4] = b"VERT";
const EDGES: &[u8; 4] = b"EDGE";
const LAYERS: &[u8; 4] = b"LAYR";
const VERTEX_LAYERS: &[u8; 4] = b"VLAY";
const HOLES: &[u8; 4] = b"HOLE";
const COLORS: &[u8; 4] = b"COLR";
const IMAGES: &[u8; 4] = b"IMAG";
const CURVE_FLAG: u8 = 1;
const HOLDOUT_FLAG: u8 = 2;

pub struct Meta {
	pub offset: Vec2,
	pub scale: f32,
}

pub struct Loaded {
	pub path: PathBuf,
	pub mesh: Mesh,
	pub meta: Option<Meta>,
	pub images: Vec<(Decoded, [Pos2; 4])>,
}

pub struct Workspace {
	pub path: Option<PathBuf>,
	saved: u64,
	saving: Option<Receiver<io::Result<(PathBuf, u64)>>>,
	loading: Option<Receiver<io::Result<Loaded>>>,
	autosave: Instant,
}

impl Workspace {
	pub fn new() -> Self {
		Self {
			path: None,
			saved: 0,
			saving: None,
			loading: None,
			autosave: Instant::now(),
		}
	}

	pub fn reset(&mut self, path: Option<PathBuf>, revision: u64) {
		self.path = path;
		self.saved = revision;
		self.autosave = Instant::now();
	}

	pub fn is_dirty(&self, revision: u64) -> bool {
		self.saved != revision
	}

	pub fn is_busy(&self) -> bool {
		self.saving.is_some() || self.loading.is_some()
	}

	pub fn autosave_due(&mut self, ctx: &egui::Context) -> bool {
		let elapsed = self.autosave.elapsed();
		if elapsed < AUTOSAVE_INTERVAL {
			ctx.request_repaint_after(AUTOSAVE_INTERVAL - elapsed);
			return false;
		}

		self.autosave = Instant::now();
		true
	}

	pub fn save(
		&mut self,
		ctx: &egui::Context,
		path: PathBuf,
		mesh: Mesh,
		meta: Meta,
		revision: u64,
	) {
		let (sender, receiver) = mpsc::channel();
		let ctx = ctx.clone();
		self.saving = Some(receiver);
		self.autosave = Instant::now();
		thread::spawn(move || {
			let result = write(&path, &mesh, &meta).map(|()| (path, revision));
			if sender.send(result).is_ok() {
				ctx.request_repaint();
			}
		});
	}

	pub fn load(&mut self, ctx: &egui::Context, path: PathBuf) {
		let (sender, receiver) = mpsc::channel();
		let ctx = ctx.clone();
		let max_side = images::max_side(&ctx);
		self.loading = Some(receiver);
		thread::spawn(move || {
			let result = read(path, max_side);
			if sender.send(result).is_ok() {
				ctx.request_repaint();
			}
		});
	}

	pub fn poll_save(&mut self) -> Option<io::Result<()>> {
		let result = poll(&mut self.saving)?.map(|(path, revision)| {
			self.path = Some(path);
			self.saved = revision;
		});
		Some(result)
	}

	pub fn poll_load(&mut self) -> Option<io::Result<Loaded>> {
		poll(&mut self.loading)
	}
}

fn poll<T>(receiver: &mut Option<Receiver<io::Result<T>>>) -> Option<io::Result<T>> {
	let result = match receiver.as_ref()?.try_recv() {
		Ok(result) => result,
		Err(TryRecvError::Empty) => return None,
		Err(TryRecvError::Disconnected) => Err(io::Error::other("background task stopped")),
	};
	*receiver = None;
	Some(result)
}

fn write(path: &Path, mesh: &Mesh, meta: &Meta) -> io::Result<()> {
	let temp = path.with_extension(format!("{EXTENSION}.tmp"));
	fs::write(&temp, encode(mesh, meta))?;
	fs::rename(&temp, path)
}

fn read(path: PathBuf, max_side: u32) -> io::Result<Loaded> {
	let bytes = fs::read(&path)?;
	let (mesh, meta, sources) = decode(&bytes)
		.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid workspace file"))?;

	let images = thread::scope(|scope| {
		let handles: Vec<_> = sources
			.into_iter()
			.map(|(corners, source)| {
				scope.spawn(move || {
					images::decode(source, max_side).map(|decoded| (decoded, corners))
				})
			})
			.collect();
		handles
			.into_iter()
			.filter_map(|handle| handle.join().ok().flatten())
			.collect()
	});

	Ok(Loaded {
		path,
		mesh,
		meta,
		images,
	})
}

fn encode(mesh: &Mesh, meta: &Meta) -> Vec<u8> {
	let mut writer = Writer::default();
	writer.bytes.extend_from_slice(MAGIC);
	writer.u32(VERSION);

	writer.section(META, |writer| {
		writer.f32(meta.offset.x);
		writer.f32(meta.offset.y);
		writer.f32(meta.scale);
	});
	writer.section(VERTICES, |writer| {
		writer.u32(mesh.vertices.len() as u32);
		mesh.vertices.iter().for_each(|&pos| writer.pos(pos));
	});
	writer.section(EDGES, |writer| {
		writer.u32(mesh.edges.len() as u32);
		mesh.edges
			.iter()
			.flatten()
			.for_each(|&index| writer.u32(index as u32));
	});
	writer.section(LAYERS, |writer| {
		writer.u32(mesh.layers.len() as u32);
		for layer in &mesh.layers {
			writer.u32(layer.id);
			writer.u8((layer.curve as u8 * CURVE_FLAG) | (layer.holdout as u8 * HOLDOUT_FLAG));
			writer.u32(layer.name.len() as u32);
			writer.bytes.extend_from_slice(layer.name.as_bytes());
		}
	});
	writer.section(VERTEX_LAYERS, |writer| {
		writer.u32(mesh.vertex_layers.len() as u32);
		mesh.vertex_layers.iter().for_each(|&id| writer.u32(id));
	});
	writer.section(HOLES, |writer| {
		writer.u32(mesh.holes.len() as u32);
		mesh.holes.iter().for_each(|hole| writer.indices(hole));
	});
	writer.section(COLORS, |writer| {
		writer.u32(mesh.colors.len() as u32);
		for (face, color) in &mesh.colors {
			writer.indices(face);
			writer.bytes.extend_from_slice(&color.to_array());
		}
	});
	writer.section(IMAGES, |writer| {
		writer.u32(mesh.images.len() as u32);
		for image in &mesh.images {
			image.corners.iter().for_each(|&pos| writer.pos(pos));
			writer.u64(image.source.len() as u64);
			writer.bytes.extend_from_slice(&image.source);
		}
	});
	writer.bytes
}

type Sources = Vec<([Pos2; 4], Arc<[u8]>)>;

fn decode(bytes: &[u8]) -> Option<(Mesh, Option<Meta>, Sources)> {
	let mut reader = Reader { bytes };
	if &reader.array::<4>()? != MAGIC || reader.u32()? > VERSION {
		return None;
	}

	let mut mesh = Mesh::default();
	let mut meta = None;
	let mut sources = Vec::new();
	while !reader.bytes.is_empty() {
		let tag = reader.array::<4>()?;
		let len = usize::try_from(reader.u64()?).ok()?;
		let mut section = Reader {
			bytes: reader.take(len)?,
		};

		match &tag {
			META => {
				meta = Some(Meta {
					offset: vec2(section.f32()?, section.f32()?),
					scale: section.f32()?,
				})
			}
			VERTICES => mesh.vertices = section.list(8, Reader::pos)?,
			EDGES => {
				mesh.edges = section.list(8, |reader| Some([reader.index()?, reader.index()?]))?
			}
			LAYERS => {
				mesh.layers = section.list(9, |reader| {
					let id = reader.u32()?;
					let flags = reader.u8()?;
					let len = reader.u32()? as usize;
					let name = String::from_utf8(reader.take(len)?.to_vec()).ok()?;
					Some(Layer {
						id,
						curve: flags & CURVE_FLAG != 0,
						holdout: flags & HOLDOUT_FLAG != 0,
						name,
					})
				})?
			}
			VERTEX_LAYERS => mesh.vertex_layers = section.list(4, Reader::u32)?,
			HOLES => mesh.holes = section.list(4, Reader::indices)?,
			COLORS => {
				mesh.colors = section.list(8, |reader| {
					let face = reader.indices()?;
					let [r, g, b, a] = reader.array()?;
					Some((face, Color32::from_rgba_premultiplied(r, g, b, a)))
				})?
			}
			IMAGES => {
				sources = section.list(40, |reader| {
					let corners = [reader.pos()?, reader.pos()?, reader.pos()?, reader.pos()?];
					let len = usize::try_from(reader.u64()?).ok()?;
					Some((corners, Arc::from(reader.take(len)?)))
				})?
			}
			_ => {}
		}
	}

	is_valid(&mesh).then_some((mesh, meta, sources))
}

fn is_valid(mesh: &Mesh) -> bool {
	let in_range = |&index: &usize| index < mesh.vertices.len();
	mesh.vertex_layers.len() == mesh.vertices.len()
		&& mesh
			.vertex_layers
			.iter()
			.all(|&id| mesh.layers.iter().any(|layer| layer.id == id))
		&& mesh.edges.iter().flatten().all(in_range)
		&& mesh.holes.iter().flatten().all(in_range)
		&& mesh.colors.iter().flat_map(|(face, _)| face).all(in_range)
}

#[derive(Default)]
struct Writer {
	bytes: Vec<u8>,
}

impl Writer {
	fn u8(&mut self, value: u8) {
		self.bytes.push(value);
	}

	fn u32(&mut self, value: u32) {
		self.bytes.extend_from_slice(&value.to_le_bytes());
	}

	fn u64(&mut self, value: u64) {
		self.bytes.extend_from_slice(&value.to_le_bytes());
	}

	fn f32(&mut self, value: f32) {
		self.bytes.extend_from_slice(&value.to_le_bytes());
	}

	fn pos(&mut self, pos: Pos2) {
		self.f32(pos.x);
		self.f32(pos.y);
	}

	fn indices(&mut self, indices: &[usize]) {
		self.u32(indices.len() as u32);
		indices.iter().for_each(|&index| self.u32(index as u32));
	}

	fn section(&mut self, tag: &[u8; 4], write: impl FnOnce(&mut Self)) {
		self.bytes.extend_from_slice(tag);
		let start = self.bytes.len();
		self.u64(0);
		write(self);
		let len = (self.bytes.len() - start - 8) as u64;
		self.bytes[start..start + 8].copy_from_slice(&len.to_le_bytes());
	}
}

struct Reader<'a> {
	bytes: &'a [u8],
}

impl<'a> Reader<'a> {
	fn take(&mut self, len: usize) -> Option<&'a [u8]> {
		let (head, rest) = self.bytes.split_at_checked(len)?;
		self.bytes = rest;
		Some(head)
	}

	fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
		self.take(N)?.try_into().ok()
	}

	fn u8(&mut self) -> Option<u8> {
		Some(self.array::<1>()?[0])
	}

	fn u32(&mut self) -> Option<u32> {
		Some(u32::from_le_bytes(self.array()?))
	}

	fn u64(&mut self) -> Option<u64> {
		Some(u64::from_le_bytes(self.array()?))
	}

	fn f32(&mut self) -> Option<f32> {
		Some(f32::from_le_bytes(self.array()?))
	}

	fn pos(&mut self) -> Option<Pos2> {
		Some(pos2(self.f32()?, self.f32()?))
	}

	fn index(&mut self) -> Option<usize> {
		Some(self.u32()? as usize)
	}

	fn indices(&mut self) -> Option<Vec<usize>> {
		self.list(4, Reader::index)
	}

	fn list<T>(
		&mut self,
		size: usize,
		mut read: impl FnMut(&mut Self) -> Option<T>,
	) -> Option<Vec<T>> {
		let count = self.u32()? as usize;
		if count.checked_mul(size)? > self.bytes.len() {
			return None;
		}

		(0..count).map(|_| read(self)).collect()
	}
}
