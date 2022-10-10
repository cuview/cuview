use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use super::common::AnvilRegion;
use super::WorldLoader;
use crate::types::shared::Shared;
use crate::types::{ChunkPos, RegionPos, ResourceLocation};
use crate::world;

struct Loader {
	root: PathBuf,
}

impl WorldLoader for Loader {
	fn load_world(&self) -> Shared<world::World> {
		world::World::new(&self.root)
	}

	fn load_dimension(
		&self,
		world: &Shared<world::World>,
		id: ResourceLocation,
	) -> Shared<world::Dimension> {
		// FIXME: move somewhere shared probably
		let dimDir = match (id.modid.as_str(), id.name.as_str()) {
			("minecraft", "overworld") => ".",
			("minecraft", "the_end") => "DIM1",
			("minecraft", "the_nether") => "DIM-1",
			("minecraft", name) => panic!("Unknown vanilla dimension `minecraft:{}`", name),
			_ => todo!("handle modded dimensions"),
		};
		let mut world = world.borrow_mut();
		let rootDir = world.root_dir().join(dimDir);
		dbg!((&dimDir, &rootDir));
		world.new_dimension(id, &rootDir)
	}

	fn load_region(
		&self,
		dimension: &Shared<world::Dimension>,
		pos: RegionPos,
	) -> Shared<world::Region> {
		let region = dimension.borrow_mut().new_region(pos);
		region
	}

	fn load_chunk(&self, region: &Shared<world::Region>, pos: ChunkPos) -> Shared<world::Chunk> {
		let chunk = region.borrow_mut().new_chunk(pos);
		chunk
	}
}

pub fn make_loader(root: &Path) -> Box<dyn WorldLoader> {
	Box::new(Loader {
		root: root.to_owned(),
	})
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDat {
	#[serde(rename = "Data")]
	pub vanillaData: LevelDatVanillaData,

	#[serde(rename = "fml")]
	pub forgeData: Option<LevelDatForgeData>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDatVanillaData {
	pub levelName: String,
	pub time: i64,

	pub spawnX: i32,
	pub spawnY: i32,
	pub spawnZ: i32,

	pub serverBrands: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDatForgeData {
	pub registries: nbt::Map<String, LevelDatForgeRegistry>,
	pub loadingModList: Vec<LevelDatForgeMod>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LevelDatForgeRegistry {
	pub ids: Vec<LevelDatForgeRegistryEntry>,
	// TODO: overrides, each entry maps a resource loc to modid (block name is reused)
	// TODO: aliases/dummied, format (and purpose of dummied) unknown; need to trawl Forge source
}

#[derive(Clone, Debug, Deserialize)]
pub struct LevelDatForgeRegistryEntry {
	#[serde(rename = "K")]
	pub name: String,

	#[serde(rename = "V")]
	pub id: i32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDatForgeMod {
	pub modId: String,
	pub modVersion: String,
}

// #[derive(Clone, Debug, Deserialize)]
// pub struct ChunkWrapper {
// 	#[serde(rename = "Level")]
// 	pub level: Chunk,
// }

#[derive(Clone, Debug, Deserialize)]
pub struct Chunk {
	// #[serde(rename = "Sections")]
	pub sections: Vec<ChunkSection>,

	#[serde(rename = "LastUpdate")]
	pub lastUpdate: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChunkSection {
	#[serde(rename = "Y")]
	pub y: i8,

	#[serde(rename = "block_states")]
	pub blocks: Option<ChunkBlocks>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChunkBlocks {
	#[serde(rename = "data")]
	pub blockArray: Option<Vec<i64>>,
	pub palette: Vec<BlockState>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BlockState {
	pub name: String,
	pub properties: Option<nbt::Map<String, String>>,
}
