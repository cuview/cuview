use std::{
	collections::{BTreeMap, BTreeSet, HashMap, HashSet},
	fmt::Write,
	hash::Hash,
	ops::{Deref, DerefMut},
	path::{Path, PathBuf},
	sync::Arc,
};

use anyhow::{Context, anyhow};
use bytemuck::{Pod, Zeroable};
use glam::{BVec3, Mat4, Vec2, Vec3, vec3};
use serde::Deserialize;

use super::texture::{Cartographer, TextureId};
use crate::{
	jarfs::JarFS,
	loader::model::{
		Axis,
		BlockStateModel,
		Element,
		JsonBlockState,
		JsonModel,
		MultipartCase,
		MultipartWhen,
		OneOrMany,
		Rotation,
	},
	types::{
		IString,
		ResourceLocation,
		blockstate::{BlockState, BlockStateBuilder, BlockStateCache},
		resource_location::ResourceKind,
		shared::Shared,
	},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
	#[serde(alias = "top")]
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
	pub mins: Vec3,
	pub maxs: Vec3,
}

impl Cube {
	pub fn new(p1: Vec3, p2: Vec3) -> Self {
		let mins = p1.min(p2);
		let maxs = p1.max(p2);
		Self { mins, maxs }
	}

	pub fn from_points(pts: impl IntoIterator<Item = Vec3>) -> Self {
		let mut mins = Vec3::splat(f32::INFINITY);
		let mut maxs = Vec3::splat(f32::NEG_INFINITY);
		for pt in pts {
			mins = mins.min(pt);
			maxs = maxs.max(pt);
		}
		Self { mins, maxs }
	}

	pub fn size(&self) -> Vec3 {
		self.maxs - self.mins
	}

	pub fn transform(&self, mat: Mat4) -> Self {
		Self::from_points(self.corners().map(move |pt| mat.transform_point3(pt)))
	}

	pub fn corners(&self) -> [Vec3; 8] {
		[
			self.mins,
			Vec3::select(BVec3::new(false, true, true), self.mins, self.maxs),
			Vec3::select(BVec3::new(true, false, true), self.mins, self.maxs),
			Vec3::select(BVec3::new(false, false, true), self.mins, self.maxs),
			Vec3::select(BVec3::new(true, true, false), self.mins, self.maxs),
			Vec3::select(BVec3::new(false, true, false), self.mins, self.maxs),
			Vec3::select(BVec3::new(true, false, false), self.mins, self.maxs),
			self.maxs,
		]
	}

	pub fn vertices(&self, dir: Direction) -> [Vertex; 4] {
		let Self { mins, maxs } = self;
		match dir {
			Direction::Up => [
				Vertex {
					pos: [maxs.x, maxs.y, mins.z],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [mins.x, maxs.y, mins.z],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [maxs.x, maxs.y, maxs.z],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [mins.x, maxs.y, maxs.z],
					uv: [0.0, 0.0],
				},
			],
			Direction::Down => [
				Vertex {
					pos: [mins.x, mins.y, mins.z],
					uv: [0.0, 0.0],
				},
				Vertex {
					pos: [maxs.x, mins.y, mins.z],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [mins.x, mins.y, maxs.z],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [maxs.x, mins.y, maxs.z],
					uv: [1.0, 1.0],
				},
			],
			Direction::North => [
				Vertex {
					pos: [mins.x, maxs.y, mins.z],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [maxs.x, maxs.y, mins.z],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [mins.x, mins.y, mins.z],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [maxs.x, mins.y, mins.z],
					uv: [0.0, 0.0],
				},
			],
			Direction::East => [
				Vertex {
					pos: [maxs.x, maxs.y, mins.z],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [maxs.x, maxs.y, maxs.z],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [maxs.x, mins.y, mins.z],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [maxs.x, mins.y, maxs.z],
					uv: [0.0, 0.0],
				},
			],
			Direction::South => [
				Vertex {
					pos: [maxs.x, maxs.y, maxs.z],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [mins.x, maxs.y, maxs.z],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [maxs.x, mins.y, maxs.z],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [mins.x, mins.y, maxs.z],
					uv: [0.0, 0.0],
				},
			],
			Direction::West => [
				Vertex {
					pos: [mins.x, maxs.y, maxs.z],
					uv: [1.0, 1.0],
				},
				Vertex {
					pos: [mins.x, maxs.y, mins.z],
					uv: [0.0, 1.0],
				},
				Vertex {
					pos: [mins.x, mins.y, maxs.z],
					uv: [1.0, 0.0],
				},
				Vertex {
					pos: [mins.x, mins.y, mins.z],
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

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct FullVertex {
	pub vert: Vertex,
	pub tex_id: u32,
}

impl Deref for FullVertex {
	type Target = Vertex;

	fn deref(&self) -> &Self::Target {
		&self.vert
	}
}

#[derive(Clone, Copy)]
pub enum Texture {
	Slot(IString),
	Asset(ResourceLocation),
}

impl From<&str> for Texture {
	fn from(str: &str) -> Self {
		if let Some(stripped) = str.strip_prefix("#") {
			Self::Slot(stripped.into())
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
pub struct Model {
	pub id: ResourceLocation,
	pub parent: Option<ResourceLocation>,
	pub texture_slots: BTreeMap<IString, Texture>,
	pub faces: Vec<Face>,
}

impl Model {
	pub fn texture(&self, slot: &str) -> ResourceLocation {
		let res = (|| {
			let mut tex = self.texture_slots.get(slot)?;
			for _ in 0 .. 100 {
				match tex {
					Texture::Asset(loc) => return Some(*loc),
					Texture::Slot(name) => {
						tex = self.texture_slots.get(name)?;
					},
				}
			}
			eprintln!(
				"lookup of texture slot `{}` on model `{}` exceeded 100 iterations",
				slot, self.id
			);
			None
		})();
		res.unwrap_or_else(|| "cuview:missing_texture".into())
	}

	pub fn transform(&mut self, mat: Mat4) {
		for face in &mut self.faces {
			for vert in &mut face.verts {
				vert.pos = mat.transform_point3(vert.pos.into()).into();
			}
		}
	}

	pub fn transformed(&self, mat: Mat4) -> Self {
		let mut res = self.clone();
		res.transform(mat);
		res
	}
}

pub struct ModelCache(BTreeMap<ResourceLocation, Model>);

impl Default for ModelCache {
	fn default() -> Self {
		Self::new()
	}
}

impl ModelCache {
	const PLACEHOLDER_MODEL_IDS: &'static [&'static str] = &[
		"cuview:missing_model",
		"block/entity",
		"builtin/entity",
		"builtin/generated",
		"forge:block/default",
		"forge:item/default",
		"twilightforest:util/block_no_ao",
	];

	pub fn new() -> Self {
		ModelCache(BTreeMap::new())
	}

	pub fn from_jsons(fs: &JarFS) -> Self {
		let parse_model = |path: &Path| {
			let (loc, _) = ResourceLocation::from_path(path);
			let ctx = format!("parsing json model `{loc}` ({path:?})");
			// first parsing as a `Value` allows duplicate fields (some mods have
			// copypasta'd models...)
			let json: serde_json::Value =
				serde_json::from_str(&fs.read_text(path).context(ctx.clone()).unwrap())
					.context(ctx.clone())
					.unwrap();
			let model: JsonModel = serde_json::from_value(json).context(ctx).unwrap();
			(loc, model)
		};

		let mut jsons = HashMap::new();
		for path in fs.files(ResourceKind::Model) {
			let (loc, model) = parse_model(&path);
			jsons.insert(loc, model);
		}

		// load any inherited models that lie outside the block models directory
		let placeholders: Vec<ResourceLocation> = Self::PLACEHOLDER_MODEL_IDS
			.iter()
			.copied()
			.map(Into::into)
			.collect();
		let mut parents: HashSet<_> = jsons
			.values()
			.filter_map(|m| {
				m.parent.and_then(|id| {
					(!jsons.contains_key(&id) && !placeholders.contains(&id)).then_some(id)
				})
			})
			.collect();
		let mut new_parents = HashSet::new();
		loop {
			if parents.is_empty() {
				break;
			}

			for &parent in &parents {
				let path = parent.into_path(ResourceKind::Model);
				let (_, model) = parse_model(&path);
				if let Some(new_parent) = model.parent &&
					!jsons.contains_key(&new_parent) &&
					!placeholders.contains(&new_parent)
				{
					new_parents.insert(new_parent);
				}
				jsons.insert(parent, model);
			}
			parents.clear();
			std::mem::swap(&mut parents, &mut new_parents);
		}

		let mut cache = Self(BTreeMap::new());

		for id in placeholders {
			cache.insert(id, Model {
				id,
				parent: None,
				texture_slots: BTreeMap::new(),
				faces: vec![],
			});
		}

		let mut remaining: HashSet<_> = jsons.keys().cloned().collect();
		let mut new_models = Vec::with_capacity(remaining.len());
		let mut remaining_len = usize::MAX;
		loop {
			let new_remaining_len = remaining.len();
			if new_remaining_len == 0 {
				break;
			}
			if remaining_len == new_remaining_len {
				let remaining: BTreeSet<_> = remaining.into_iter().collect();
				panic!("Failed to load any remaining models: {remaining:?}");
			}
			remaining_len = new_remaining_len;

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
				let parent = json.parent.and_then(|p| cache.get(&p));

				let mut texture_slots = parent
					.map(|p| p.texture_slots.clone())
					.unwrap_or_else(BTreeMap::new);
				if let Some(textures) = &json.textures {
					for (k, v) in textures {
						texture_slots.insert(*k, v.as_str().into());
					}
				}

				let mut faces: Vec<Face>;
				if let Some(elems) = &json.elements {
					faces = Vec::with_capacity(elems.len() * 6);
					for elem in elems {
						let mut cube =
							Cube::new(Vec3::from(elem.from) / 16.0, Vec3::from(elem.to) / 16.0);

						let rotation = elem.rotation.map(|rot| {
							let origin = Vec3::from(rot.origin) / 16.0;
							let angle = rot.angle.to_radians();
							let rot = match rot.axis {
								Axis::X => Mat4::from_rotation_x(angle),
								Axis::Y => Mat4::from_rotation_y(angle),
								Axis::Z => Mat4::from_rotation_z(angle),
							};
							// TODO: rescale
							Mat4::from_translation(origin) * rot * Mat4::from_translation(-origin)
						});

						for (&dir, face) in &elem.faces {
							let mut verts = cube.vertices(dir);

							if let Some(rot) = rotation {
								for vert in &mut verts {
									vert.pos = rot.transform_point3(Vec3::from(vert.pos)).into();
								}
							}

							if let Some(rect) = face.uv {
								let mins = Vec2::new(rect[0], rect[1]) / 16.0;
								let maxs = Vec2::new(rect[2], rect[3]) / 16.0;
								for vert in &mut verts {
									vert.uv = (mins + (maxs - mins) * Vec2::from(vert.uv)).into();
								}
							}

							faces.push(Face {
								texture: face.texture.as_str().into(),
								verts,
							});
						}
					}
				} else {
					faces = parent
						.map(|v| v.faces.clone())
						.unwrap_or_else(std::vec::Vec::new);
				}

				new_models.push((loc, Model {
					id: loc,
					parent: json.parent,
					texture_slots,
					faces,
				}));
			}

			for (loc, model) in new_models.drain(..) {
				cache.insert(loc, model);
				remaining.remove(&loc);
			}
		}
		cache
	}

	pub fn models_using_texture(
		&self,
		target_texure: ResourceLocation,
	) -> HashSet<ResourceLocation> {
		self.values()
			.flat_map(|m| m.texture_slots.values().map(|t| (m.id, t)))
			.filter_map(|(model_id, tex)| match tex {
				Texture::Asset(id) => (*id == target_texure).then_some(model_id),
				_ => None,
			})
			.collect()
	}

	pub fn all_block_textures(&self) -> HashSet<ResourceLocation> {
		self.values()
			.flat_map(|m| m.texture_slots.values())
			.filter_map(|t| match t {
				Texture::Slot(_) => None,
				Texture::Asset(id) => Some(*id),
			})
			.collect()
	}

	pub fn geometry_buffer(&self, cartographer: &Cartographer) -> GeometryBuffer {
		let mut vertices = vec![];
		let mut model_info = HashMap::new();

		let mut vertex_id = 0;
		for (&id, model) in self.0.iter() {
			let base_vertex = vertex_id;
			let num_vertices = model.faces.len() * 6;
			vertex_id += num_vertices;
			vertices.extend(model.faces.iter().flat_map(|face| {
				let slot = match face.texture {
					Texture::Asset(_) => panic!(),
					Texture::Slot(name) => name.as_str(),
				};
				let tex_id = cartographer
					.id_for_texture(model.texture(slot))
					.unwrap_or_else(|| {
						cartographer
							.id_for_texture("cuview:missing_texture".into())
							.expect("Missing texture is itself missing! D:")
					})
					.packed();
				[
					// expand triangle strip to pair of tris with slot
					FullVertex {
						vert: face.verts[0],
						tex_id,
					},
					FullVertex {
						vert: face.verts[1],
						tex_id,
					},
					FullVertex {
						vert: face.verts[2],
						tex_id,
					},
					FullVertex {
						vert: face.verts[1],
						tex_id,
					},
					FullVertex {
						vert: face.verts[3],
						tex_id,
					},
					FullVertex {
						vert: face.verts[2],
						tex_id,
					},
				]
			}));
			model_info.insert(id, (base_vertex, num_vertices));
		}

		GeometryBuffer {
			vertices,
			model_info,
		}
	}
}

impl Deref for ModelCache {
	type Target = BTreeMap<ResourceLocation, Model>;

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
	pub vertices: Vec<FullVertex>,

	pub model_info: HashMap<ResourceLocation, (usize, usize)>,
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
	let mut blockstate_jsons = HashMap::new();
	for block in blockstates.blocks() {
		let path = block.into_path(ResourceKind::BlockState);
		let json = fs.read_text(&path);
		if json.is_err() {
			eprintln!("Warning: no blockstate json for {block}");
			continue;
		}
		let json: JsonBlockState = serde_json::from_str(&json.unwrap())
			.unwrap_or_else(|_| panic!("Malformed blockstate json for {block}"));
		blockstate_jsons.insert(block, json);
	}

	let missing = BlockStateModel {
		model: "cuview:missing".into(),
		x_rotation: None,
		y_rotation: None,
		uvlock: None,
		weight: None,
	};
	let mut models_for_state = HashMap::new();
	for state in blockstates.states() {
		let block = state.block_name();
		let mut models = vec![];
		let json = blockstate_jsons.get(&block);

		if let Some(json) = json {
			match json {
				JsonBlockState::Variants(map) => {
					let missing = OneOrMany::One(missing);
					let state_models = (|| {
						if let Some(xs) = map.get("") {
							assert!(
								map.len() == 1,
								"variants-style stateless property found among other properties \
								 in blockstate JSON for {block}"
							);
							xs
						} else {
							for (state_str, state_models) in map {
								let partial_state = BlockStateBuilder::from_variants_model(
									block,
									state_str.as_str(),
								);
								if partial_state.keys().all(|key| {
									state.get_property(key) == partial_state.get_property(key)
								}) {
									return state_models;
								}
							}
							&missing
						}
					})();
					models.push(state_models.iter().copied().collect());
				},
				JsonBlockState::Multipart(parts) => {
					let case_matches = |case: &MultipartCase| -> bool {
						for (k, vs) in &case.0 {
							let expected = state.get_property(k).unwrap_or_else(|| {
								panic!(
									"Blockstate JSON for {block} matches on property `{k}` which \
									 is not defined in blockstate dump"
								)
							});
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

		if models.is_empty() || json.is_none() {
			if json.is_some() {
				// eprintln!("Blockstate JSON has no mapping for state
				// {state}");
			}
			models.push(vec![missing]);
		}
		models_for_state.insert(state, models);
	}

	models_for_state
}

pub fn export_wavefront(models: &[(&str, &Model)], mtl_filename: &str) -> (String, String) {
	const PALETTE: &[u32] = &[
		0x0000FF, 0x00FF00, 0x00FFFF, 0xFF0000, 0xFF00FF, 0xFFFF00, 0xFFFFFF, 0x7FFF00, 0xFF7F00,
		0x007FFF, 0x00FF7F, 0x7F00FF, 0xFF007F,
	];

	let mut obj = String::new();
	let mut mtl = String::new();
	obj.write_fmt(format_args!("mtllib {mtl_filename}\n\n"))
		.unwrap();

	let mut vert_index = 1;
	let mut tex_index = 0;
	let mut slot_counts: HashMap<IString, usize> = HashMap::new();
	for (index, (model_name, model)) in models.iter().copied().enumerate() {
		if index > 0 {
			obj.write_str("\n").unwrap();
		}
		obj.write_fmt(format_args!("o {model_name}\n")).unwrap();

		let mut texgroups = HashMap::new();
		for face in model.faces.iter() {
			let tex_name: String = match face.texture {
				Texture::Slot(name) => model.texture(&name),
				Texture::Asset(loc) => loc,
			}
			.into();
			let list = texgroups
				.entry(tex_name)
				.or_insert_with(|| Vec::with_capacity(64));
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
			let tex_id = *slot_counts
				.entry(texture)
				.and_modify(|v| *v += 1)
				.or_insert(0);
			mtl.write_fmt(format_args!("newmtl {texture}{tex_id}\n"))
				.unwrap();
			mtl.write_fmt(format_args!("d 1\nNs 0\n")).unwrap();

			let color = PALETTE[tex_index % PALETTE.len()];
			tex_index += 1;
			let (r, g, b) = (
				((color & 0xFF0000) >> 16) as f32 / 255.0,
				((color & 0x00FF00) >> 8) as f32 / 255.0,
				(color & 0x0000FF) as f32 / 255.0,
			);
			mtl.write_fmt(format_args!("Kd {r:.3} {g:.3} {b:.3}\n"))
				.unwrap();
			// TODO: export textures
			// mtl.write_fmt(format_args!("map_Kd {texture}.png\n")).unwrap();

			obj.write_fmt(format_args!("usemtl {texture}{tex_id}\n"))
				.unwrap();
			for face in faces {
				let base_vert = vert_index;
				vert_index += 4;
				obj.write_fmt(format_args!(
					"f {0}/{0} {1}/{1} {2}/{2}\nf {1}/{1} {3}/{3} {2}/{2}\n",
					base_vert,
					base_vert + 1,
					base_vert + 2,
					base_vert + 3
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
