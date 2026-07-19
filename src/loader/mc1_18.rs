use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use super::{
	WorldLoader,
	common::{AnvilRegion, biterator},
};
use crate::{
	types::{ChunkPos, RegionPos, ResourceLocation, blockstate::BlockStateBuilder, shared::Shared},
	world,
};

struct Loader;

impl WorldLoader for Loader {
	fn load_chunk(
		&self,
		chunk: &Shared<world::Chunk>,
		pos: ChunkPos,
		anvil: std::sync::Arc<AnvilRegion>,
	) {
		let raw_chunk: Chunk = anvil.load_chunk(pos).unwrap();
		for raw_section in &raw_chunk.sections {
			if raw_section.blocks.is_none() {
				continue;
			}

			let block_info = raw_section.blocks.as_ref().unwrap();
			let palette: world::Palette = block_info
				.palette
				.iter()
				.map(|raw_bs| {
					let mut state = BlockStateBuilder::new(raw_bs.name.as_str().into());
					if let Some(props) = raw_bs.properties.as_ref() {
						for (k, v) in props {
							state.set_property(k.as_str().into(), v.as_str().into());
						}
					}
					state.build()
				})
				.collect();
			let palette_bits = palette.bits();

			let section = chunk.borrow_mut().new_section(raw_section.y, palette);
			if let Some(blocks) = &block_info.blocks_array {
				section
					.borrow_mut()
					.fill_from_iter(biterator(palette_bits, bytemuck::cast_slice(blocks)));
			} else {
				let it = std::iter::once(0).cycle().take(4096);
				section.borrow_mut().fill_from_iter(it);
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
	pub vanilla_data: LevelDatVanillaData,

	#[serde(rename = "fml")]
	pub forge_data: Option<LevelDatForgeData>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDatVanillaData {
	pub level_name: String,
	pub time: i64,

	pub spawn_x: i32,
	pub spawn_y: i32,
	pub spawn_z: i32,

	pub server_brands: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDatForgeData {
	pub registries: nbt::Map<String, LevelDatForgeRegistry>,
	pub loading_mod_list: Vec<LevelDatForgeMod>,
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
	pub mod_id: String,
	pub mod_version: String,
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
	pub last_update: i64,
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
	pub blocks_array: Option<Vec<i64>>,
	pub palette: Vec<BlockState>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BlockState {
	pub name: String,
	pub properties: Option<nbt::Map<String, String>>,
}
