use std::{
	fs::{File, FileType, read_dir},
	path::{Path, PathBuf},
	sync::Arc,
};

use anyhow::anyhow;

use self::common::AnvilRegion;
use crate::{
	types::{ChunkPos, RegionPos, ResourceLocation, shared::Shared},
	world::{Chunk, Dimension, Region, World},
};

pub mod blockstate;
pub mod common;
pub mod mc1_18;
pub mod model;

pub struct WorldWrangler {
	root_dir: PathBuf,
	loader: Box<dyn WorldLoader>,
	world: Shared<World>,
}

impl WorldWrangler {
	pub fn new(world_root_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
		let world_root_dir = world_root_dir.as_ref();
		let loader = get_loader(world_root_dir)?;
		let world = World::new(world_root_dir);
		loader.load_world(&world);
		Ok(Self {
			root_dir: world_root_dir.into(),
			loader,
			world,
		})
	}

	pub fn probe_dimensions(&self) -> Vec<(ResourceLocation, PathBuf)> {
		let mut dimensions = vec![
			("overworld".into(), self.root_dir.join(".")),
			("the_end".into(), self.root_dir.join("DIM1")),
			("the_nether".into(), self.root_dir.join("DIM-1")),
		];
		dimensions.extend(self.loader.probe_mod_dimensions(&self.world));
		dimensions
	}

	pub fn probe_dimension(&self, id: ResourceLocation) -> Option<(ResourceLocation, PathBuf)> {
		for (other, path) in self.probe_dimensions() {
			if id == other {
				return Some((id, path));
			}
		}
		None
	}

	pub fn load_dimension(&self, probed: (ResourceLocation, PathBuf)) -> Shared<Dimension> {
		let (id, root) = probed;
		let dimension = self.world.borrow_mut().new_dimension(id, &root);
		self.loader.load_dimension(&dimension, id, &root);
		dimension
	}

	pub fn probe_regions(&self, dimension: &Shared<Dimension>) -> Vec<RegionPos> {
		let mut res = Vec::with_capacity(32usize.pow(2));
		let region_dir = dimension.borrow().region_dir();
		let dir = read_dir(&region_dir)
			.unwrap_or_else(|_| panic!("could not read region dir `{region_dir:?}`"));
		for entry in dir {
			if entry.is_err() {
				continue;
			}
			let entry = entry.unwrap();

			let meta = entry.metadata();
			if meta.is_err() {
				continue;
			}
			let meta = meta.unwrap();

			if !meta.is_file() || meta.len() == 0 {
				continue;
			}

			let name = entry.file_name();
			let name = name.to_str();
			if name.is_none() {
				continue;
			}
			let name = name.unwrap();

			let mut coords = name
				.splitn(4, ".")
				.skip(1)
				.take(2)
				.map(|str| str.parse::<i32>().unwrap());
			let x = coords.next().unwrap();
			let y = coords.next().unwrap();
			res.push(RegionPos::new(x, y));
		}
		res
	}

	pub fn load_region(&self, dimension: &Shared<Dimension>, pos: RegionPos) -> Shared<Region> {
		let region = dimension.borrow_mut().new_region(pos);
		self.loader.load_region(&region, pos);
		region
	}

	pub fn probe_chunks(&self, region: &Shared<Region>) -> Vec<ChunkPos> {
		let (anvil, pos) = {
			let region = region.borrow();
			(region.anvil(), region.pos())
		};
		pos.chunks().filter(|pos| !anvil.is_empty(*pos)).collect()
	}

	pub fn load_chunk(&self, region: &Shared<Region>, pos: ChunkPos) -> Shared<Chunk> {
		let (anvil, chunk) = {
			let mut region = region.borrow_mut();
			(region.anvil(), region.new_chunk(pos))
		};
		self.loader.load_chunk(&chunk, pos, anvil);
		chunk
	}
}

pub trait WorldLoader {
	fn load_world(&self, world: &Shared<World>) {}

	fn probe_mod_dimensions(&self, world: &Shared<World>) -> Vec<(ResourceLocation, PathBuf)> {
		vec![]
	}

	fn load_dimension(&self, dimension: &Shared<Dimension>, id: ResourceLocation, root: &Path) {}

	fn load_region(&self, region: &Shared<Region>, pos: RegionPos) {}

	fn load_chunk(&self, chunk: &Shared<Chunk>, pos: ChunkPos, anvil: Arc<AnvilRegion>);
}

pub fn identify_version(world_root: impl AsRef<Path>) -> Option<(u8, u8, u8)> {
	let mut level_dat = File::open(world_root.as_ref().join("level.dat")).ok()?;
	let nbt: nbt::Blob = nbt::from_gzip_reader(&mut level_dat).ok()?;
	let nbt = nbt.get("Data")?;

	let ver = match nbt {
		nbt::Value::Compound(map) => map.get("Version"),
		_ => None,
	}?;
	let ver = match ver {
		nbt::Value::Compound(map) => map.get("Name"),
		_ => None,
	}?;
	let ver = match ver {
		nbt::Value::String(s) => Some(s),
		_ => None,
	}?;

	let (v1, rest) = ver.split_once(".")?;
	let (v2, v3) = rest.split_once(".").unwrap_or((rest, "0"));
	Some((v1.parse().ok()?, v2.parse().ok()?, v3.parse().ok()?))
}

pub fn get_loader(world_root_dir: impl AsRef<Path>) -> anyhow::Result<Box<dyn WorldLoader>> {
	let world_root = world_root_dir.as_ref();
	if let Some(ver) = identify_version(world_root) {
		return match ver {
			(1, 18, _) => Ok(mc1_18::make_loader(world_root)),
			(1, 17, _) => Ok(mc1_18::make_loader(world_root)), // FIXME
			(1, 16, _) => Ok(mc1_18::make_loader(world_root)),
			_ => Err(anyhow!(
				"Couldn't find any loader for `{world_root:?}` (version {ver:?})",
			)),
		};
	}
	Err(anyhow!(
		"Couldn't identify Minecraft version of `{world_root:?}`",
	))
}
