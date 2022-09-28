#![allow(non_snake_case, unused)]

use std::collections::{BTreeSet, HashSet};
use std::convert::TryInto;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::exit;

use cuview::loader::common::AnvilRegion;
use cuview::loader::model::{Element, Face as JsonFace, JsonModel, MergedModel, ModelCache};
use cuview::loader::*;
use cuview::renderer::model::{Cube, Direction, Face, Model, Vertex};
use cuview::types::blockstate::BlockState;
use cuview::types::{BlockPos, ChunkPos, IString, RegionPos, ResourceLocation};
use cuview::world::Palette;
use glam::Vec3;

#[cfg(none)]
fn main() {
	/* let mut diagonal = Model::new(None);
	diagonal.textures.insert("tex".into(), ResourceLocation::from("cuview:test"));
	diagonal.faces.push(Face {
		texture: "tex".into(),
		verts: [
			Vertex {
				pos: [1.0, 1.0, 1.0],
				uv: [0.0, 0.0],
			},
			Vertex {
				pos: [0.0, 1.0, 1.0],
				uv: [0.0, 0.0],
			},
			Vertex {
				pos: [1.0, 0.0, 0.0],
				uv: [0.0, 0.0],
			},
			Vertex {
				pos: [0.0, 0.0, 0.0],
				uv: [0.0, 0.0],
			},
		]
	}); */

	/*let cube = Cube::new(Vec3::splat(-0.5), Vec3::splat(0.5));
	let mut test = Model::new(None);
	for dir in { use Direction::*; [Up, Down, North, East, South, West] } {
		let tex = format!("{dir:?}").into();
		test.textures.insert(tex, ResourceLocation::from(format!("cuview:test_{dir:?}").as_str()));
		test.faces.push(Face {
			texture: tex,
			verts: cube.vertices(dir),
		});
	}*/

	let mut raw = MergedModel::new();
	raw.textures.insert("foo".into(), "cuview:foo".into());
	raw.elements.push(Element {
		from: [0.0, 0.0, 0.0],
		to: [16.0, 16.0, 16.0],
		faces: [
			// #[cfg(none)]
			(
				Direction::Up,
				JsonFace {
					texture: "foo".into(),
					..Default::default()
				},
			),
			// #[cfg(none)]
			(
				Direction::Down,
				JsonFace {
					texture: "foo".into(),
					..Default::default()
				},
			),
		]
		.into(),
		shade: true,
		rotation: None,
	});

	let test = Model::bake(&raw);
	dbg!(test.faces.len());
	let (obj, mtl) = Model::into_wavefront(&[("test", test)], "test.mtl");
	std::fs::write("out/test.obj", obj);
	std::fs::write("out/test.mtl", mtl);
}

// #[cfg(none)]
fn main() {
	let fs = cuview::jarfs::JarFS::new(vec![
		Path::new("client-1.18.2.jar"),
		// Path::new("snad.jar"),
	])
	.unwrap();

	let mut cache = ModelCache::new();
	cache.load_jsons(&fs);
	dbg!(cache.jsons.len());
	cache.merge_jsons();
	dbg!(cache.merged.len());
	// dbg!(cache.merged.keys().filter(|&&k|
	// k.name.contains("fence_post")).collect::<Vec<_>>()); dbg!(cache.
	// get_model(ResourceLocation::from("block/stone_slab_top")));

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
	let mut baked = Vec::with_capacity(interestingModels.len());
	for (modelIndex, modelPath) in interestingModels.iter().cloned().enumerate() {
		let loc = ResourceLocation::from(modelPath);
		let raw = cache.get_model(loc).expect(&format!("{modelPath}"));
		let mut model = Model::bake(raw);
		for face in &mut model.faces {
			for vert in &mut face.verts {
				vert.pos = [
					vert.pos[0] + modelIndex as f32,
					vert.pos[1],
					vert.pos[2],
				];
			}
		}
		baked.push((modelPath, model));
	}

	let (obj, mtl) = Model::into_wavefront(baked.as_slice(), "interesting.mtl");
	std::fs::write("out/interesting.obj", obj).unwrap();
	std::fs::write("out/interesting.mtl", mtl).unwrap();

	/* let files = fs.all_files();
	let mc = files
		.iter()
		.filter(|v| v.starts_with("assets/minecraft/blockstates"))
		.count();
	let snad = files
		.iter()
		.filter(|v| v.starts_with("assets/snad/blockstates"))
		.count(); */
	/* let owo = files
	.iter()
	.filter(|&v| {
		if let Ok(dirs) = <Vec<_> as TryInto<[&str; 4]>>::try_into(v.splitn(4, "/").collect()) {
			match dirs {
				["assets", _, "blockstates", _] => true,
				_ => false,
			}
		} else {
			false
		}
	})
	// .take(5)
	.count(); */
	/*let owo = files
		.iter()
		.filter(|&v| {
			let components: Vec<_> = v
				.components()
				.map(|v| v.as_os_str().to_str().unwrap())
				.collect();
			match components.as_slice() {
				["assets", _, "blockstates", ..] => true,
				_ => false,
			}
		})
		// .take(5)
		.count();
	eprintln!("{mc}\n{snad}\n{owo}");*/
}

#[cfg(none)]
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

	// #[cfg(none)]
	{
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
	}
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

#[cfg(none)]
pub fn parse_nbt_value<T: DeserializeOwned>(v: &nbt::Value) -> Result<T, nbt::Error> {
	use serde::de::DeserializeOwned;
	let mut buf = Vec::new();
	buf.resize(v.len_bytes(), Default::default());
	v.to_writer(&mut buf)?;
	nbt::from_reader(buf.as_slice())
}
