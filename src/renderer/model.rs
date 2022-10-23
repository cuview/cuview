use std::collections::{HashMap, HashSet, BTreeSet};
use std::fmt::Write;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use anyhow::Context;
use bytemuck::{Zeroable, Pod};
use glam::{Vec2, Vec3, Mat4};
use serde::Deserialize;

use crate::jarfs::JarFS;
use crate::loader::model::{
	BlockStateModel,
	JsonBlockState,
	MultipartCase,
	MultipartWhen,
	OneOrMany, JsonModel, Element,
};
use crate::types::blockstate::{BlockState, BlockStateBuilder, BlockStateCache};
use crate::types::resource_location::ResourceKind;
use crate::types::shared::Shared;
use crate::types::{IString, ResourceLocation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
	Up,

	#[serde(alias = "bottom")]
	Down,

	North,

	East,

	South,

	West,
}

#[derive(Clone, Copy)]
pub struct Cube {
	mins: Vec3,
	maxs: Vec3,
}

impl Cube {
	pub fn new(mins: Vec3, maxs: Vec3) -> Self {
		Self { mins, maxs }
	}

	pub fn vertices(&self, dir: Direction) -> [Vertex; 4] {
		let Self { mins, maxs } = self;
		match dir {
			Direction::Up => [
				Vertex {
					pos: [
						maxs.x, maxs.y, mins.z,
					],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [
						mins.x, maxs.y, mins.z,
					],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [
						maxs.x, maxs.y, maxs.z,
					],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [
						mins.x, maxs.y, maxs.z,
					],
					uv: [0.0, 0.0],
				},
			],
			Direction::Down => [
				Vertex {
					pos: [
						mins.x, mins.y, mins.z,
					],
					uv: [0.0, 0.0],
				},
				Vertex {
					pos: [
						maxs.x, mins.y, mins.z,
					],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [
						mins.x, mins.y, maxs.z,
					],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [
						maxs.x, mins.y, maxs.z,
					],
					uv: [1.0, 1.0],
				},
			],
			Direction::North => [
				Vertex {
					pos: [
						mins.x, maxs.y, mins.z,
					],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [
						maxs.x, maxs.y, mins.z,
					],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [
						mins.x, mins.y, mins.z,
					],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [
						maxs.x, mins.y, mins.z,
					],
					uv: [0.0, 0.0],
				},
			],
			Direction::East => [
				Vertex {
					pos: [
						maxs.x, maxs.y, mins.z,
					],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [
						maxs.x, maxs.y, maxs.z,
					],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [
						maxs.x, mins.y, mins.z,
					],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [
						maxs.x, mins.y, maxs.z,
					],
					uv: [0.0, 0.0],
				},
			],
			Direction::South => [
				Vertex {
					pos: [
						maxs.x, maxs.y, maxs.z,
					],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [
						mins.x, maxs.y, maxs.z,
					],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [
						maxs.x, mins.y, maxs.z,
					],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [
						mins.x, mins.y, maxs.z,
					],
					uv: [0.0, 0.0],
				},
			],
			Direction::West => [
				Vertex {
					pos: [
						mins.x, maxs.y, maxs.z,
					],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [
						mins.x, maxs.y, mins.z,
					],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [
						mins.x, mins.y, maxs.z,
					],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [
						mins.x, mins.y, mins.z,
					],
					uv: [0.0, 0.0],
				},
			],
		}
	}
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vertex {
	pub pos: [f32; 3],
	pub uv: [f32; 2],
}

#[derive(Clone, Copy)]
pub enum Texture {
	Slot(IString),
	Asset(ResourceLocation),
}

impl From<&str> for Texture {
	fn from(str: &str) -> Self {
		if str.starts_with("#") {
			Self::Slot((&str[1 ..]).into())
		} else {
			Self::Asset(str.into())
		}
	}
}

#[derive(Clone, Copy)]
pub struct Face {
	pub verts: [Vertex; 4],
	pub texture: Texture,
}

#[derive(Clone)]
pub enum Faces {
	Inherited(ResourceLocation),
	Specified(Shared<Vec<Face>>),
}

impl Faces {
	pub fn inherited(&self) -> bool {
		match self {
			Self::Inherited(_) => true,
			_ => false,
		}
	}
}

#[derive(Clone)]
pub struct Model {
	id: ResourceLocation,
	parent: Option<ResourceLocation>,
	textureSlots: HashMap<IString, Texture>,
	faces: Faces,
}

impl Model {
	pub fn into_wavefront(cache: &ModelCache, models: &[(&str, Self)], mtlFilename: &str) -> (String, String) {
		const palette: &[u32] = &[
			0x0000FF, 0x00FF00, 0x00FFFF, 0xFF0000, 0xFF00FF, 0xFFFF00, 0xFFFFFF, 0x7FFF00,
			0xFF7F00, 0x007FFF, 0x00FF7F, 0x7F00FF, 0xFF007F,
		];

		let mut obj = String::new();
		let mut mtl = String::new();
		obj.write_fmt(format_args!("mtllib {mtlFilename}\n\n"))
			.unwrap();

		let mut vertIndex = 1;
		let mut texIndex = 0;
		let mut slotCounts: HashMap<IString, usize> = HashMap::new();
		for (index, (modelName, model)) in models.iter().enumerate() {
			if index > 0 {
				obj.write_str("\n").unwrap();
			}
			obj.write_fmt(format_args!("o {modelName}\n")).unwrap();

			let mut texgroups = HashMap::new();
			let faces = model.faces(cache);
			let faces = faces.borrow();
			for face in faces.iter() {
				let texName: String = match face.texture {
					Texture::Slot(name) => model.texture(&name),
					Texture::Asset(loc) => loc,
				}.into();
				let list = if let Some(v) = texgroups.get_mut(&texName) {
					v
				} else {
					texgroups.insert(texName.clone(), Vec::with_capacity(128));
					texgroups.get_mut(&texName).unwrap()
				};
				list.push(face);
			}

			for (texture, faces) in texgroups {
				let texture = texture
					.chars()
					.map(|c| match c {
						'a' ..= 'z' | 'A' ..= 'Z' => c,
						_ => '_',
					})
					.collect::<String>()
					.into();
				let texId = if let Some(&v) = slotCounts.get(&texture) {
					slotCounts.insert(texture, v + 1);
					v
				} else {
					slotCounts.insert(texture, 1);
					0usize
				};
				mtl.write_fmt(format_args!("newmtl {texture}{texId}\n"))
					.unwrap();
				mtl.write_fmt(format_args!("d 1\nNs 0\n")).unwrap();

				let color = palette[texIndex % palette.len()];
				texIndex += 1;
				let (r, g, b) = (
					((color & 0xFF0000) >> 16) as f32 / 255.0,
					((color & 0x00FF00) >> 8) as f32 / 255.0,
					((color & 0x0000FF) >> 0) as f32 / 255.0,
				);
				mtl.write_fmt(format_args!("Kd {r:.3} {g:.3} {b:.3}\n"))
					.unwrap();
				// TODO: export textures
				// mtl.write_fmt(format_args!("map_Kd {texture}.png\n")).unwrap();

				obj.write_fmt(format_args!("usemtl {texture}{texId}\n"))
					.unwrap();
				for face in faces {
					let baseVert = vertIndex;
					vertIndex += 4;
					obj.write_fmt(format_args!(
						"f {0}/{0} {1}/{1} {2}/{2}\nf {1}/{1} {3}/{3} {2}/{2}\n",
						baseVert + 0,
						baseVert + 1,
						baseVert + 2,
						baseVert + 3
					))
					.unwrap();
					for vert in face.verts {
						obj.write_fmt(format_args!(
							"v {:.3} {:.3} {:.3}\n",
							vert.pos[0], vert.pos[1], vert.pos[2]
						))
						.unwrap();
						obj.write_fmt(format_args!("vt {:.3} {:.3}\n", vert.uv[0], vert.uv[1]))
							.unwrap();
					}
				}
			}
		}

		(obj, mtl)
	}
	
	pub fn texture(&self, slot: &str) -> ResourceLocation {
		let res = (|| {
			let mut tex = self.textureSlots.get(slot)?;
			for _ in 0 .. 100 {
				match tex {
					Texture::Asset(loc) => return Some(*loc),
					Texture::Slot(name) => { tex = self.textureSlots.get(name)?; },
				}
			}
			eprintln!("lookup of texture slot `{}` on model `{}` exceeded 100 iterations", slot, self.id);
			None
		})();
		res.unwrap_or_else(|| "cuview:missing_texture".into())
	}
	
	pub fn faces<'a>(&self, cache: &ModelCache) -> Shared<Vec<Face>> {
		match &self.faces {
			Faces::Specified(faces) => faces.clone(),
			Faces::Inherited(src) => {
				let src = cache.get(src).unwrap();
				match &src.faces {
					Faces::Inherited(_) => panic!(),
					Faces::Specified(faces) => faces.clone(),
				}
			}
		}
	}
	
	pub fn transformed(&self, cache: &ModelCache, mat: Mat4) -> Self {
		let mut res = self.clone();
		let mut faces = res.faces(cache).borrow().clone();
		for face in &mut faces {
			for vert in &mut face.verts {
				vert.pos = mat.transform_point3(vert.pos.into()).into();
			}
		}
		res.faces = Faces::Specified(faces.into());
		res
	}
}

pub struct ModelCache(HashMap<ResourceLocation, Model>);

impl ModelCache {
	pub fn new() -> Self {
		ModelCache(HashMap::new())
	}
	
	pub fn from_jsons(fs: &JarFS) -> Self {
		let mut jsons = HashMap::new();
		for path in fs.files(ResourceKind::Model) {
			let (loc, _) = ResourceLocation::from_path(&path);
			let model: JsonModel = serde_json::from_str(&fs.read_text(&path).unwrap()).context(format!("parsing json model `{loc}`")).unwrap();
			jsons.insert(loc, model);
		}
		
		let mut cache: HashMap<ResourceLocation, Model> = HashMap::new();
		let mut remaining: HashSet<_> = jsons.keys().cloned().collect();
		let mut newModels = Vec::with_capacity(remaining.len());
		let mut remainingLen = usize::MAX;
		loop {
			let newRemainingLen = remaining.len();
			if newRemainingLen == 0 {
				break;
			}
			if remainingLen == newRemainingLen {
				panic!("Failed to load any remaining models: {remaining:?}");
			}
			remainingLen = newRemainingLen;

			for loc in remaining.iter().cloned().filter(|loc| {
				let json = jsons.get(loc).unwrap();
				if let Some(parent) = json.parent {
					// models with a parent must be deferred until that parent has been parsed
					cache.contains_key(&parent)
				} else {
					// models without parents can be immediately parsed
					true
				}
			}) {
				let json = jsons.get(&loc).unwrap();
				let parent = json.parent.map(|p| cache.get(&p)).flatten();

				let mut textureSlots = parent.map(|p| p.textureSlots.clone()).unwrap_or_else(|| HashMap::new());
				if let Some(textures) = &json.textures {
					for (k, v) in textures {
						textureSlots.insert(k.clone(), v.as_str().into());
					}
				}
				
				let mut faces: Faces;
				if let Some(elems) = &json.elements {
					let mut newFaces = Vec::with_capacity(elems.len() * 6);
					for elem in elems {
						let cube = Cube::new(Vec3::from(elem.from) / 16.0, Vec3::from(elem.to) / 16.0);
						for (&dir, face) in &elem.faces {
							let mut verts = cube.vertices(dir);
							if let Some(rect) = face.uv {
								let mins = Vec2::new(rect[0], rect[1]) / 16.0;
								let maxs = Vec2::new(rect[2], rect[3]) / 16.0;
								for vert in &mut verts {
									vert.uv = (mins + (maxs - mins) * Vec2::from(vert.uv)).into();
								}
							}
							newFaces.push(Face {
								texture: face.texture.as_str().into(),
								verts,
							});
						}
					}
					faces = Faces::Specified(newFaces.into());
				} else {
					let facesSrc = (|| if let Some(parent) = parent {
						let mut src = parent;
						while src.faces.inherited() {
							if let Some(parentLoc) = src.parent {
								if let Some(parent) = cache.get(&parentLoc) {
									src = parent;
								} else {
									break;
								}
							} else {
								break;
							}
						}
						
						if !src.faces.inherited() {
							return Some(src.id);
						}
						None
					} else {
						None
					})();
					faces = Faces::Inherited(facesSrc.unwrap_or_else(|| "cuview:error".into()));
				}

				newModels.push((loc, Model {
					id: loc,
					parent: json.parent,
					textureSlots,
					faces,
				}));
			}
			for (loc, model) in newModels.drain(..) {
				cache.insert(loc, model);
				remaining.remove(&loc);
			}
		}
		Self(cache)
	}
	
	pub fn geometry_buffer(&self) -> GeometryBuffer {
		let mut vertices = vec![];
		let mut modelMap = HashMap::new();
		let mut inheritingModels = HashSet::new();
		
		let mut vertexId = 0;
		for (&id, model) in self.0.iter() {
			match &model.faces {
				Faces::Inherited(_) => {
					inheritingModels.insert(id);
					continue;
				},
				Faces::Specified(faces) => {
					let faces = faces.borrow();
					let baseVertex = vertexId;
					let numVertices = faces.len() * 6;
					vertexId += numVertices;
					vertices.extend(faces.iter().flat_map(|face| [
						// expand triangle strip to pair of tris
						face.verts[0],
						face.verts[1],
						face.verts[2],
						face.verts[1],
						face.verts[3],
						face.verts[2],
					]));
					modelMap.insert(id, (baseVertex, numVertices));
				}
			}
		}
		
		for id in inheritingModels {
			let model = self.0.get(&id).unwrap();
			match &model.faces {
				Faces::Specified(_) => unreachable!(),
				Faces::Inherited(parent) => {
					if let Some(info) = modelMap.get(parent).cloned() {
						modelMap.insert(id, info);
					}
					// TODO: logging `None`s?
				}
			}
		}
		
		GeometryBuffer {
			vertices,
			modelMap
		}
	}
}

impl Deref for ModelCache {
	type Target = HashMap<ResourceLocation, Model>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for ModelCache {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

pub struct GeometryBuffer {
	pub vertices: Vec<Vertex>,
	
	pub modelMap: HashMap<ResourceLocation, (usize, usize)>
}

/**
	Determines for every posssible blockstate the set of models which should be rendered for that state.

	The interior `Vec`s are lists of models to be randomly chosen from; the outer `Vec`s are a set
	of (chosen) models which should all be rendered together for the given state.
*/
pub fn models_for_states(
	fs: &JarFS,
	blockstates: &BlockStateCache,
) -> HashMap<BlockState, Vec<Vec<BlockStateModel>>> {
	let mut blockstateJsons = HashMap::new();
	for block in blockstates.blocks() {
		let path = block.into_path(ResourceKind::BlockState);
		let json = fs.read_text(&path);
		if json.is_err() {
			eprintln!("Warning: no blockstate json for {block}");
			continue;
		}
		let json: JsonBlockState = serde_json::from_str(&json.unwrap())
			.expect(&format!("Malformed blockstate json for {block}"));
		blockstateJsons.insert(block, json);
	}

	let missing = BlockStateModel {
		model: "cuview:missing".into(),
		xRotation: None,
		yRotation: None,
		uvlock: None,
		weight: None,
	};
	let mut modelsForState = HashMap::new();
	for state in blockstates.states() {
		let block = state.block_name();
		let mut models = vec![];
		let json = blockstateJsons.get(&block);

		if let Some(json) = json {
			match json {
				JsonBlockState::Variants(map) => {
					let missing = OneOrMany::One(missing);
					let stateModels = (|| {
						if let Some(xs) = map.get("") {
							assert!(
								map.len() == 1,
								"variants-style stateless property found among other properties \
								 in blockstate JSON for {block}"
							);
							xs
						} else {
							for (stateStr, stateModels) in map {
								let partialState = BlockStateBuilder::from_variants_model(
									block,
									stateStr.as_str(),
								);
								if partialState.keys().all(|key| {
									state.get_property(key) == partialState.get_property(key)
								}) {
									return stateModels;
								}
							}
							&missing
						}
					})();
					models.push(stateModels.iter().copied().collect());
				},
				JsonBlockState::Multipart(parts) => {
					let case_matches = |case: &MultipartCase| -> bool {
						for (k, vs) in &case.0 {
							let expected = state.get_property(&k).expect(&format!(
								"Blockstate JSON for {block} matches on property `{k}` which is \
								 not defined in blockstate dump"
							));
							if vs.0.iter().all(|v| v != expected) {
								return false;
							}
						}

						true
					};

					for part in parts {
						let mut matches = true;
						if let Some(when) = &part.when {
							match when {
								MultipartWhen::And(case) => {
									matches = case_matches(case);
								},
								MultipartWhen::Or { or: cases } => {
									matches = false;
									for case in cases {
										matches |= case_matches(case);
										if matches {
											break;
										}
									}
								},
							}
						}

						if matches {
							models.push(part.apply.iter().copied().collect());
						}
					}
				},
			}
		}

		if models.len() == 0 || json.is_none() {
			if json.is_some() {
				// eprintln!("Blockstate JSON has no mapping for state {state}");
			}
			models.push(vec![missing]);
		}
		modelsForState.insert(state, models);
	}

	modelsForState
}
