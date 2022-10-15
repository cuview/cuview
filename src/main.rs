#![allow(non_snake_case, unused)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::TryInto;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::exit;

use blockstate::BlockStates;
use cuview::loader::common::AnvilRegion;
use cuview::loader::model::{
	Element,
	Face as JsonFace,
	JsonBlockState,
	JsonModel,
};
use cuview::loader::{self, *};
use cuview::renderer::model::{models_for_states, ModelCache, Model};
use cuview::types::blockstate::{BlockState, BlockStateBuilder, BlockStateCache};
use cuview::types::resource_location::ResourceKind;
use cuview::types::{BlockPos, ChunkPos, IString, RegionPos, ResourceLocation};
use cuview::world::Palette;
use glam::{Vec3, Mat4, vec3};
use loader::model::{BlockStateModel, MultipartCase, OneOrMany};
use model::MultipartWhen;

#[cfg(none)]
fn main() {
	let fs = cuview::jarfs::JarFS::new(vec![
		Path::new("client-1.18.2.jar"),
		// Path::new("snad.jar"),
	])
	.unwrap();

	let mut blockstates: blockstate::BlockStates =
		serde_json::from_str(&std::fs::read_to_string("blockstates.json").unwrap()).unwrap();
	/* dbg!(
		&blockstates
			.0
			.get(&"redstone_wire".into())
			.unwrap()
			.properties
	); */
	// blockstates.0.retain(|&k, _| k.name.as_str() == "sandstone_wall");
	/* let k = blockstates.0.keys().copied().next().unwrap();
	blockstates.0.get_mut(&k).unwrap().states.truncate(1); */
	#[cfg(none)]
	blockstates.0.insert(
		"cuview:test".into(),
		blockstate::BlockDefinition {
			properties: None,
			states: vec![blockstate::State {
				properties: None,
				id: u32::MAX,
				default: true,
			}],
		},
	);
	let blockstates = BlockStateCache::from_json(blockstates);

	let modelsForState = models_for_states(&fs, &blockstates);
	let test1 = BlockState::stateless("stone".into());
	let test2 = BlockStateBuilder::from_variants_model("grass_block".into(), "snowy=false").build();
	let test3 = BlockStateBuilder::from_variants_model(
		"cobblestone_wall".into(),
		"north=low,east=none,south=none,west=none,up=true,waterlogged=false",
	)
	.build();
	dbg!(test1, modelsForState.get(&test1));
	dbg!(test2, modelsForState.get(&test2));
	dbg!(test3, modelsForState.get(&test3));
}

#[cfg(none)]
fn main() {
	let fs = cuview::jarfs::JarFS::new(vec![
		Path::new("client-1.18.2.jar"),
		// Path::new("snad.jar"),
	])
	.unwrap();
	let mut modelCache = ModelCache::from_jsons(&fs);

	let interestingModels = [
		"block/cactus",
		"block/fence_post",
		"block/fence_side",
		"block/template_fence_gate_wall",
		"block/template_fence_gate_open",
		"block/cross",
		"block/slab_top",
		"block/slab",
		"block/stairs",
		"block/stonecutter",
	];
	let mut xformed = Vec::with_capacity(interestingModels.len());
	for (modelIndex, modelPath) in interestingModels.iter().cloned().enumerate() {
		let loc = ResourceLocation::from(modelPath);
		let mat = Mat4::from_translation(vec3(modelIndex as f32, 0.0, 0.0));
		let model = modelCache.get(&loc).expect(&format!("{modelPath}")).transformed(&modelCache, mat);
		xformed.push((modelPath, model));
	}

	let (obj, mtl) = Model::into_wavefront(&modelCache, xformed.as_slice(), "interesting.mtl");
	std::fs::write("out/interesting.obj", obj).unwrap();
	std::fs::write("out/interesting.mtl", mtl).unwrap();
}

// #[cfg(none)]
fn main() {
	let args: Vec<_> = std::env::args().collect();
	if args.len() < 2 {
		let procName = &args[0];
		eprintln!("Usage: {procName} <path to world>");
		exit(1);
	}

	let worldDir = PathBuf::from(args[1].as_str());
	if !worldDir.is_dir() {
		let worldDir = worldDir.display();
		eprintln!("{worldDir} is not a directory");
		exit(1);
	}

	let version = identify_version(&worldDir);
	if version == None {
		eprintln!("Couldn't determine Minecraft version of the given world");
		exit(1);
	}
	let version = version.unwrap();
	println!(
		"Minecraft version: {}.{}.{}",
		version.0, version.1, version.2
	);

	let wrangler = WorldWrangler::new(worldDir).unwrap();

	let dim = wrangler.probe_dimensions().iter().filter(|&&(id, _)| id == "overworld".into()).cloned().next().unwrap();
	let dim = wrangler.load_dimension(dim);
	
	#[cfg(none)]
	{
		// searching for chunks using global palette
		let regions = loader.probe_regions(&dim);
		for (ri, &regionPos) in regions.iter().enumerate() {
			let region = loader.load_region(&dim, regionPos);
			let anvil = region.borrow_mut().load_anvil().unwrap();
			let chunks = loader.probe_chunks(&region);
			for (ci, &chunkPos) in chunks.iter().enumerate() {
				// let chunk: nbt::Blob = anvil.load_chunk(chunkPos).unwrap();
				// dbg!(&chunk);
				// todo!();
				let chunk: mc1_18::Chunk = anvil.load_chunk(chunkPos).unwrap();
				if ci % 8 == 0 || ci == chunks.len() - 1 {
					eprint!(
						"\rregion {}/{} ({},{}) chunk {}/{}            ",
						ri + 1,
						regions.len(),
						regionPos.x,
						regionPos.z,
						ci + 1,
						chunks.len()
					);
				}
			}
			eprintln!();
		}

		return;
	}
	
	// #[cfg(none)]
	{
		// let targetBlock = BlockPos::new(0, -61, 0);
		let targetBlock = BlockPos::new(-1, -48, 0);
		// let targetBlock = BlockPos::new(13 * 16, -60, 13 * 16);

		let region = wrangler.load_region(&dim, targetBlock.into());
		let chunk = wrangler.load_chunk(&region, targetBlock.into());
		let section = chunk.borrow().get_section(targetBlock.section()).unwrap();
		dbg!(section.borrow().get_block(targetBlock));
		dbg!(
			ChunkPos::from(targetBlock).blocks_in_section(targetBlock.section()).map(|pos| section.borrow().get_block(pos)).filter(|v| 
				v.block_name() != "air".into()
			).collect::<Vec<_>>()
		);
	}
}

#[cfg(none)]
pub fn parse_nbt_value<T: DeserializeOwned>(v: &nbt::Value) -> Result<T, nbt::Error> {
	use serde::de::DeserializeOwned;
	let mut buf = Vec::new();
	buf.resize(v.len_bytes(), Default::default());
	v.to_writer(&mut buf)?;
	nbt::from_reader(buf.as_slice())
}
