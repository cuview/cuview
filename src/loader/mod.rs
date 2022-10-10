use std::fs::{read_dir, File, FileType};
use std::path::{Path, PathBuf};

use self::common::AnvilRegion;
use crate::types::shared::Shared;
use crate::types::{ChunkPos, RegionPos, ResourceLocation};
use crate::world::{Chunk, Dimension, Region, World};

pub mod blockstate;
pub mod common;
pub mod mc1_18;
pub mod model;

pub trait WorldLoader {
	fn load_world(&self) -> Shared<World>;

	fn probe_dimensions(&self, world: &Shared<World>) -> Vec<ResourceLocation> {
		vec![
			"overworld".into(),
			"the_nether".into(),
			"the_end".into(),
		]
	}

	fn load_dimension(&self, world: &Shared<World>, id: ResourceLocation) -> Shared<Dimension>;

	fn probe_regions(&self, dimension: &Shared<Dimension>) -> Vec<RegionPos> {
		let mut res = Vec::with_capacity(32 * 32);
		let regionDir = dimension.borrow().region_dir();
		let dir = read_dir(dimension.borrow().region_dir())
			.expect(&format!("could not read region dir `{:?}`", regionDir));
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

	fn load_region(&self, dimension: &Shared<Dimension>, pos: RegionPos) -> Shared<Region>;

	fn probe_chunks(&self, region: &Shared<Region>) -> Vec<ChunkPos> {
		let (regionDir, regionPos) = {
			let region = region.borrow();
			(region.dimension().borrow().region_dir(), region.pos())
		};
		let anvil = AnvilRegion::new(regionDir, regionPos).unwrap(); // FIXME: cache in `Region`?
		regionPos
			.chunks()
			.filter(|pos| !anvil.is_empty(*pos))
			.collect()
	}

	fn load_chunk(&self, region: &Shared<Region>, pos: ChunkPos) -> Shared<Chunk>;
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

pub fn get_loader(worldRoot: impl AsRef<Path>) -> Result<Box<dyn WorldLoader>, String> {
	let worldRoot = worldRoot.as_ref();
	if let Some(ver) = identify_version(worldRoot) {
		return match ver {
			(1, 18, _) => Ok(mc1_18::make_loader(worldRoot)),
			(1, 17, _) => Ok(mc1_18::make_loader(worldRoot)), // FIXME
			(1, 16, _) => Ok(mc1_18::make_loader(worldRoot)),
			_ => Err(format!(
				"Couldn't find any loader for `{:?}` (version {:?})",
				worldRoot, ver
			)),
		};
	}
	Err(format!(
		"Couldn't identify Minecraft version of `{:?}`",
		worldRoot
	))
}
