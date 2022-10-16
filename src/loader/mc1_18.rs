use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use super::common::{AnvilRegion, biterator};
use super::WorldLoader;
use crate::types::blockstate::BlockStateBuilder;
use crate::types::shared::Shared;
use crate::types::{ChunkPos, RegionPos, ResourceLocation};
use crate::world;

struct Loader;

impl WorldLoader for Loader {
	fn load_chunk(&self, chunk: &Shared<world::Chunk>, pos: ChunkPos, anvil: std::sync::Arc<AnvilRegion>) {
		let rawChunk: Chunk = anvil.load_chunk(pos).unwrap();
		for rawSection in &rawChunk.sections {
			if rawSection.blocks.is_none() {
				chunk.borrow_mut().new_section(rawSection.y, world::Palette::new());
				continue;
			}
			
			let blockInfo = rawSection.blocks.as_ref().unwrap();
			let palette: world::Palette = blockInfo.palette.iter().map(|rawBS| {
				let mut state = BlockStateBuilder::new(rawBS.name.as_str().into());
				if let Some(props) = rawBS.properties.as_ref() {
					for (k, v) in props {
						state.set_property(k.as_str().into(), v.as_str().into());
					}
				}
				state.build()
			}).collect();
			let paletteBits = palette.bits();
			
			let section = chunk.borrow_mut().new_section(rawSection.y, palette);
			if let Some(blocks) = &blockInfo.blockArray {
				section.borrow_mut().fill_blocks(biterator(paletteBits, bytemuck::cast_slice(blocks)));
			} else {
				let it = std::iter::once(0).cycle().take(4096);
				section.borrow_mut().fill_blocks(it);
			}
		}
	}
}

pub fn make_loader(root: &Path) -> Box<dyn WorldLoader> {
	Box::new(Loader)
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
