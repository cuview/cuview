use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use glam::Vec3;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer};
use serde_json::Value as JsonValue;

use crate::jarfs::JarFS;
use crate::renderer::model::Direction;
use crate::types::blockstate::BlockState;
use crate::types::ResourceLocation;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
	One(T),
	Many(Vec<T>),
}

struct StringVisitor(String);

impl<'de> Visitor<'de> for StringVisitor {
	type Value = String;

	fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(formatter, "a string")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: serde::de::Error,
	{
		Ok(v.to_owned())
	}
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
pub struct MultipartProp(Vec<String>);

impl From<JsonValue> for MultipartProp {
	fn from(v: JsonValue) -> Self {
		Self(v.to_string().split("|").map(ToOwned::to_owned).collect())
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct MultipartProps(HashMap<String, MultipartProp>);

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum MultipartWhen {
	Or {
		#[serde(rename = "OR")]
		or: Vec<MultipartProps>,
	},
	And(MultipartProps),
}

#[derive(Clone, Debug, Deserialize)]
pub struct BlockStateModel {
	pub model: String,

	#[serde(rename = "x")]
	pub xRotation: Option<f32>,

	#[serde(rename = "y")]
	pub yRotation: Option<f32>,

	pub uvlock: Option<bool>,

	pub weight: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JsonModel {
	pub parent: Option<String>,

	pub textures: Option<HashMap<String, String>>,

	pub elements: Option<Vec<Element>>,
	// TODO
	// pub ambientocclusion: bool;
	// pub display: HashMap<Display, DisplayPosition>;
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

#[derive(Clone, Debug, Deserialize)]
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

	pub rotation: Option<i32>,

	pub tintindex: Option<i32>,
}

pub struct ModelCache {
	pub jsons: HashMap<ResourceLocation, JsonModel>,
	pub merged: HashMap<ResourceLocation, MergedModel>,
}

impl ModelCache {
	pub fn new() -> Self {
		Self {
			jsons: HashMap::new(),
			merged: HashMap::new(),
		}
	}

	fn resource_for_zip_path(path: &Path) -> ResourceLocation {
		let modid = path
			.components()
			.skip(1)
			.take(1)
			.nth(0)
			.unwrap()
			.as_os_str()
			.to_str()
			.unwrap();
		let modelPath = path
			.components()
			.skip(3)
			.collect::<PathBuf>()
			.to_str()
			.unwrap()
			.replace(std::path::MAIN_SEPARATOR, "/");
		ResourceLocation::new(modid, &modelPath)
	}

	pub fn load_jsons(&mut self, fs: &JarFS) {
		let files = fs.all_files();
		let files: Vec<_> = files
			.iter()
			.filter(|&v| {
				let components: Vec<_> = v
					.components()
					.map(|v| v.as_os_str().to_str().unwrap())
					.collect();
				match components.as_slice() {
					["assets", _, "models", "block", ..] => true,
					_ => false,
				}
			})
			.collect();

		for path in files {
			let loc = Self::resource_for_zip_path(&path.with_extension(""));
			let model: JsonModel = serde_json::from_str(&fs.read_text(path).unwrap()).unwrap();
			self.jsons.insert(loc, model);
		}
	}

	pub fn merge_jsons(&mut self) {
		let emptyTextures = HashMap::new();
		let mut remaining: HashSet<_> = self.jsons.keys().cloned().collect();
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
				let json = self.jsons.get(loc).unwrap();
				if let Some(parent) = json.parent.as_ref() {
					// models with a parent must be deferred until that parent has been merged
					self.merged
						.contains_key(&ResourceLocation::from(parent.as_str()))
				} else {
					// models without parents can be immediately "merged"
					true
				}
			}) {
				let json = self.jsons.get(&loc).unwrap();
				let parent = json
					.parent
					.as_ref()
					.map(|p| self.merged.get(&ResourceLocation::from(p.as_str())))
					.flatten();
				let mut model = MergedModel {
					elements: json
						.elements
						.as_ref()
						.map(|v| v.clone())
						.unwrap_or_else(|| {
							parent
								.map(|p| p.elements.clone())
								.unwrap_or_else(|| Vec::new())
						}),
					textures: parent.map_or_else(|| HashMap::new(), |p| p.textures.clone()),
				};

				for (k, v) in json.textures.as_ref().unwrap_or_else(|| &emptyTextures) {
					model.textures.insert(k.clone(), v.clone());
				}

				newModels.push((loc, model));
			}
			for (loc, model) in newModels.drain(0 .. newModels.len()) {
				self.merged.insert(loc, model);
				remaining.remove(&loc);
			}
		}
	}

	pub fn get_model(&self, loc: ResourceLocation) -> Option<&MergedModel> {
		self.merged.get(&loc)
	}
}

#[derive(Clone, Debug)]
pub struct MergedModel {
	pub textures: HashMap<String, String>,
	pub elements: Vec<Element>,
}

impl MergedModel {
	pub fn new() -> Self {
		Self {
			textures: HashMap::new(),
			elements: Vec::new(),
		}
	}
}
