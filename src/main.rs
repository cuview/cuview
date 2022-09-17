#![allow(non_snake_case, unused)]

use std::io::Write;
use std::path::PathBuf;
use std::process::exit;

use cuview::loader::common::AnvilRegion;
use cuview::loader::*;
use cuview::types::blockstate::BlockState;
use cuview::types::{BlockPos, ChunkPos, IString, RegionPos, ResourceLocation};
use cuview::world::Palette;

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

	/*let levelDat = worldDir.join("level.dat");
	let levelDat: mc1_18::LevelDat = nbt::from_gzip_reader(std::fs::File::open(levelDat).unwrap()).unwrap();
	dbg!(&levelDat); */

	let loader = get_loader(worldDir).unwrap();

	let world = loader.load_world();
	dbg!(&world);
	dbg!(loader.probe_dimensions(&world));

	let dim = loader.load_dimension(&world, "overworld".into());
	dbg!(&dim);
	// dbg!(loader.probe_regions(&dim));

	let regionDir = dim.borrow().region_dir();
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
					print!(
						"\rregion {}/{} ({},{}) chunk {}/{}            ",
						ri + 1,
						regions.len(),
						regionPos.x,
						regionPos.z,
						ci + 1,
						chunks.len()
					);
					std::io::stdout().flush();
				}
			}
			println!();
		}

		return;
	}

	let air = BlockState::new(ResourceLocation::new(
		&*IString::from_static("minecraft"),
		&*IString::from_static("air"),
	));
	// let targetBlock = BlockPos::new(0, -60, 0);
	let targetBlock = BlockPos::new(13 * 16, -60, 13 * 16);

	let region = loader.load_region(&dim, targetBlock.into());
	// dbg!(&region);
	let anvil = region.borrow_mut().load_anvil().unwrap();

	// let chunk = loader.load_chunk(&region, targetBlock.into());
	let chunk: mc1_18::Chunk = anvil.load_chunk(targetBlock.into()).unwrap();
	let section = (targetBlock.section() + 5) as usize;
	dbg!(section);
	let section = &chunk.sections[section];
	let binfo = section.blocks.as_ref().unwrap();

	let mut palette = Palette::new();
	let mut id = 0;
	for rawBS in &binfo.palette {
		let mut state = BlockState::new(rawBS.name.as_str().into());
		if let Some(props) = rawBS.properties.as_ref() {
			for (k, v) in props {
				state.set_property(k.as_str().into(), v.as_str().into());
			}
		}
		palette.define(id, state);
		id += 1;
	}
	dbg!(palette.bits());
	// dbg!(&palette);

	todo!();
	let barray = binfo.blockArray.as_ref().unwrap();
	for pid in biterator(palette.bits(), unsafe {
		std::mem::transmute(barray.as_slice())
	}) {
		let state = palette.get_state(pid).unwrap();
		if state == air {
			continue;
		}
		dbg!(state);
	}

	// let region = loader.load_region(&dim, targetBlock.into());
	// region.borrow_mut().load_anvil().unwrap();

	// let chunk = loader.load_chunk(&region, targetBlock.into());
	// dbg!(chunk);
}

fn biterator<'a>(bits: usize, mut xs: &'a [u64]) -> impl Iterator<Item = u32> + 'a {
	let bits = bits as u32;
	let mask = (1 << bits) - 1;
	let mut x = xs[0];
	xs = &xs[1 ..];
	let mut remainder = u64::BITS;
	std::iter::from_fn(move || {
		if remainder == 0 && xs.len() == 0 {
			None
		} else {
			if remainder == 0 {
				x = xs[0];
				xs = &xs[1 ..];
			}

			let elem = x & mask;
			x >>= bits;
			if let Some(v) = remainder.checked_sub(bits) {
				remainder = v;
				// TODO: <=1.15 wraps entries across words
				if remainder < bits {
					remainder = 0;
				}
			} else {
				remainder = 0;
			}
			Some(elem as u32)
		}
	})
}

/* use serde::de::DeserializeOwned;
pub fn parse_nbt_value<T: DeserializeOwned>(v: &nbt::Value) -> Result<T,
nbt::Error> { let mut buf = Vec::new();
buf.resize(v.len_bytes(), Default::default());
v.to_writer(&mut buf)?;
nbt::from_reader(buf.as_slice())
} */
