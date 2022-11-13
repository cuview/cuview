use std::fs::{read_dir, File, FileType};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::anyhow;

use self::common::AnvilRegion;
use crate::types::shared::Shared;
use crate::types::{ChunkPos, RegionPos, ResourceLocation};
use crate::world::{Chunk, Dimension, Region, World};

pub mod blockstate;
pub mod common;
pub mod mc1_18;
pub mod model;

pub struct WorldWrangler {
	rootDir: PathBuf,
	loader: Box<dyn WorldLoader>,
	world: Shared<World>,
}

impl WorldWrangler {
	pub fn new(worldRootDir: impl AsRef<Path>) -> anyhow::Result<Self> {
		let worldRootDir = worldRootDir.as_ref();
		let loader = get_loader(worldRootDir)?;
		let world = World::new(worldRootDir);
		loader.load_world(&world);
		Ok(Self {
			rootDir: worldRootDir.into(),
			loader,
			world,
		})
	}

	pub fn probe_dimensions(&self) -> Vec<(ResourceLocation, PathBuf)> {
		let mut dimensions = vec![
			("overworld".into(), self.rootDir.join(".")),
			("the_end".into(), self.rootDir.join("DIM1")),
			("the_nether".into(), self.rootDir.join("DIM-1")),
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
		let regionDir = dimension.borrow().region_dir();
		let dir =
			read_dir(&regionDir).expect(&format!("could not read region dir `{regionDir:?}`"));
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

pub fn identify_version(worldRoot: impl AsRef<Path>) -> Option<(u8, u8, u8)> {
	let mut levelDat = File::open(worldRoot.as_ref().join("level.dat")).ok()?;
	let nbt: nbt::Blob = nbt::from_gzip_reader(&mut levelDat).ok()?;
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

pub fn get_loader(worldRootDir: impl AsRef<Path>) -> anyhow::Result<Box<dyn WorldLoader>> {
	let worldRoot = worldRootDir.as_ref();
	if let Some(ver) = identify_version(worldRoot) {
		return match ver {
			(1, 18, _) => Ok(mc1_18::make_loader(worldRoot)),
			(1, 17, _) => Ok(mc1_18::make_loader(worldRoot)), // FIXME
			(1, 16, _) => Ok(mc1_18::make_loader(worldRoot)),
			_ => Err(anyhow!(
				"Couldn't find any loader for `{worldRoot:?}` (version {ver:?})",
			)),
		};
	}
	Err(anyhow!(
		"Couldn't identify Minecraft version of `{worldRoot:?}`",
	))
}
