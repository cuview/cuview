#![allow(non_snake_case)]

use std::path::PathBuf;
use std::process::exit;

use cuview::loader::common::AnvilRegion;
use cuview::loader::*;
use cuview::types::{ChunkPos, RegionPos};

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

	// let loader =
	let regionDir = worldDir.join("region/");
	let anvil = AnvilRegion::new(regionDir, RegionPos::new(0, 0)).unwrap();
	// let chunk: mc1_18::ChunkWrapper = anvil.load_chunk(ChunkPos::new(0, 0)).unwrap();
	let chunk: nbt::Blob = anvil.load_chunk(ChunkPos::new(0, 0)).unwrap();
	println!("{}", chunk);
	/* let mut blah = std::fs::File::open(worldDir.join("region/r.0.0.mca")).unwrap();
	let hmm: nbt::Blob = nbt::from_reader(&mut blah).unwrap();
	dbg!(hmm); */
}

/* use serde::de::DeserializeOwned;
pub fn parse_nbt_value<T: DeserializeOwned>(v: &nbt::Value) -> Result<T,
nbt::Error> { let mut buf = Vec::new();
buf.resize(v.len_bytes(), Default::default());
v.to_writer(&mut buf)?;
nbt::from_reader(buf.as_slice())
} */
