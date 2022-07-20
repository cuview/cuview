use std::fs::File;
use std::path::Path;

use crate::types::{RegionPos, ResourceLocation};
use crate::world::{Dimension, World};
use crate::Shared;

pub mod common;
pub mod mc1_18;

pub trait WorldLoader {
	fn load_world(&self, root: &Path) -> Shared<World>;

	fn probe_dimensions(&self) -> Box<dyn Iterator<Item = ResourceLocation>>;

	fn load_dimension(&self, world: Shared<World>, id: &ResourceLocation);

	fn load_region(&self, world: Shared<World>, dimension: Shared<Dimension>, pos: RegionPos);
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
