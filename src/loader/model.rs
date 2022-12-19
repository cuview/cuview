use std::collections::{HashMap, HashSet};
use std::iter::FusedIterator;
use std::path::{Path, PathBuf};

use glam::Vec3;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer};
use serde_json::Value as JsonValue;

use crate::jarfs::JarFS;
use crate::renderer::model::Direction;
use crate::types::blockstate::BlockState;
use crate::types::resource_location::ResourceKind;
use crate::types::{IString, ResourceLocation};

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
	One(T),
	Many(Vec<T>),
}

impl<T> OneOrMany<T> {
	pub fn iter(&self) -> impl FusedIterator<Item = &T> {
		struct Iter<'a, T> {
			this: &'a OneOrMany<T>,
			index: usize,
		}

		impl<'a, T> Iterator for Iter<'a, T> {
			type Item = &'a T;

			fn next(&mut self) -> Option<Self::Item> {
				match self.this {
					OneOrMany::One(x) => {
						if self.index == 0 {
							self.index += 1;
							Some(x)
						} else {
							None
						}
					},
					OneOrMany::Many(xs) => {
						if self.index < xs.len() {
							let res = Some(&xs[self.index]);
							self.index += 1;
							res
						} else {
							None
						}
					},
				}
			}

			fn size_hint(&self) -> (usize, Option<usize>) {
				let len = match self.this {
					OneOrMany::One(_) => 1 - self.index,
					OneOrMany::Many(xs) => xs.len() - self.index,
				};
				(len, Some(len))
			}
		}

		impl<'a, T> FusedIterator for Iter<'a, T> {}

		Iter {
			this: self,
			index: 0,
		}
	}
}

#[test]
fn test_oneormany() {
	let x = OneOrMany::One(0);
	assert!(x.iter().cloned().collect::<Vec<_>>() == vec![0]);
	let x = OneOrMany::Many(vec![0, 1]);
	assert!(x.iter().cloned().collect::<Vec<_>>() == vec![0, 1]);

	let mut it = x.iter();
	assert!(it.size_hint() == (2, Some(2)));
	it.next().unwrap();
	assert!(it.size_hint() == (1, Some(1)));
	it.next().unwrap();
	assert!(it.size_hint() == (0, Some(0)));
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonBlockState {
	Variants(HashMap<String, OneOrMany<BlockStateModel>>),
	Multipart(Vec<Multipart>),
}

#[derive(Clone, Debug, Deserialize)]
pub struct Multipart {
	pub apply: OneOrMany<BlockStateModel>,

	pub when: Option<MultipartWhen>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(from = "serde_json::Value")]
pub struct MultipartProp(pub Vec<String>);

impl From<JsonValue> for MultipartProp {
	fn from(v: JsonValue) -> Self {
		let str = v.as_str().unwrap();
		Self(str.split("|").map(ToOwned::to_owned).collect())
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct MultipartCase(pub HashMap<String, MultipartProp>);

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum MultipartWhen {
	Or {
		#[serde(rename = "OR")]
		or: Vec<MultipartCase>,
	},
	And(MultipartCase),
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct BlockStateModel {
	pub model: ResourceLocation,

	#[serde(rename = "x")]
	pub xRotation: Option<f32>,

	#[serde(rename = "y")]
	pub yRotation: Option<f32>,

	pub uvlock: Option<bool>,

	pub weight: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JsonModel {
	pub parent: Option<ResourceLocation>,

	pub textures: Option<HashMap<IString, String>>,

	pub elements: Option<Vec<Element>>,
	// TODO
	// pub ambientocclusion: bool;
	// pub display: HashMap<Display, DisplayTransform>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
	X,

	Y,

	Z,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Element {
	pub from: [f32; 3],

	pub to: [f32; 3],

	pub rotation: Option<Rotation>,

	#[serde(default = "Element::default_shade")]
	pub shade: bool,

	pub faces: HashMap<Direction, Face>,
}

impl Element {
	fn default_shade() -> bool {
		true
	}
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Rotation {
	pub origin: [f32; 3],

	pub axis: Axis,

	pub angle: f32,

	#[serde(default)]
	pub rescale: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Face {
	pub texture: String,

	pub uv: Option<[f32; 4]>,

	pub cullface: Option<Direction>,

	#[serde(rename = "rotation")]
	pub textureRotation: Option<i32>,

	pub tintindex: Option<i32>,
}
