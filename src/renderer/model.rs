use std::collections::HashMap;
use std::fmt::Write;
use std::hash::Hash;

use glam::{Vec3, Vec2};
use serde::Deserialize;

use crate::loader::model::MergedModel;
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
	pub mins: Vec3,
	pub maxs: Vec3,
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

#[derive(Clone, Copy)]
pub struct Vertex {
	pub pos: [f32; 3],
	pub uv: [f32; 2],
}

#[derive(Clone, Copy)]
pub struct Face {
	pub verts: [Vertex; 4],
	pub texture: IString,
}

pub struct Model {
	pub faces: Vec<Face>,
	pub textures: HashMap<IString, ResourceLocation>,
}

impl Model {
	pub fn new(capacity: Option<(usize, usize)>) -> Self {
		let (faces, textures) = capacity.unwrap_or((0, 0));
		Self {
			faces: Vec::with_capacity(faces),
			textures: HashMap::with_capacity(textures),
		}
	}

	pub fn bake(json: &MergedModel) -> Self {
		let mut res = Self::new(Some((json.elements.len() * 6, json.textures.len())));

		for elem in &json.elements {
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
				res.faces.push(Face {
					texture: face.texture.as_str().into(),
					verts,
				});
			}
		}

		for (slot, texPath) in &json.textures {
			res.textures
				.insert(slot.as_str().into(), texPath.as_str().into());
		}

		res
	}

	pub fn into_wavefront(models: &[(&str, Self)], mtlFilename: &str) -> (String, String) {
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
			for face in &model.faces {
				let list = if let Some(v) = texgroups.get_mut(&face.texture) {
					v
				} else {
					texgroups.insert(face.texture, Vec::with_capacity(128));
					texgroups.get_mut(&face.texture).unwrap()
				};
				list.push(face);
			}

			for (slot, faces) in texgroups {
				let slot = slot
					.chars()
					.map(|c| match c {
						'a' ..= 'z' | 'A' ..= 'Z' => c,
						_ => '_',
					})
					.collect::<String>()
					.into();
				let slotId = if let Some(&v) = slotCounts.get(&slot) {
					slotCounts.insert(slot, v + 1);
					v
				} else {
					slotCounts.insert(slot, 1);
					0usize
				};
				mtl.write_fmt(format_args!("newmtl {slot}{slotId}\n"))
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

				obj.write_fmt(format_args!("usemtl {slot}{slotId}\n"))
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
}
