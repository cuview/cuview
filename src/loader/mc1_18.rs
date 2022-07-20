use serde::{Deserialize, Serialize};

use super::WorldLoader;

struct Loader {}

/*impl WorldLoader for Loader {

}

pub fn get_loader() -> impl WorldLoader {
	Loader
}*/

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDat {
	#[serde(rename = "Data")]
	pub vanillaData: LevelDatVanillaData,

	#[serde(rename = "fml")]
	pub forgeData: Option<LevelDatForgeData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDatVanillaData {
	pub levelName: String,
	pub time: i64,

	pub spawnX: i32,
	pub spawnY: i32,
	pub spawnZ: i32,

	pub serverBrands: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDatForgeData {
	pub registries: nbt::Map<String, LevelDatForgeRegistry>,
	pub loadingModList: Vec<LevelDatForgeMod>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelDatForgeRegistry {
	pub ids: Vec<LevelDatForgeRegistryEntry>,
	// TODO: overrides/dummies?
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelDatForgeRegistryEntry {
	#[serde(rename = "K")]
	pub name: String,

	#[serde(rename = "V")]
	pub id: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LevelDatForgeMod {
	pub modId: String,
	pub modVersion: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChunkWrapper {
	pub dataVersion: i32,
	pub level: Chunk,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Chunk {
	pub sections: Vec<ChunkSection>,
	pub heightMap: Vec<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChunkSection {
	pub y: i8,
	pub blocks: Vec<i8>,
}
