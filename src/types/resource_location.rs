use std::cell::RefCell;
use std::fmt::{Debug, Display};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::value::Value as JsonValue;

use super::IString;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
	BlockState,
	Model,
	Texture,
	TextureMeta,
}

impl ResourceKind {
	pub fn path_prefix(self) -> &'static str {
		match self {
			Self::BlockState => "blockstates",
			Self::Model => "models",
			Self::Texture => "textures",
			Self::TextureMeta => "textures",
		}
	}

	pub fn extension(self) -> &'static str {
		match self {
			Self::BlockState => "json",
			Self::Model => "json",
			Self::Texture => "png",
			Self::TextureMeta => "png.mcmeta",
		}
	}
}

impl From<&str> for ResourceKind {
	fn from(str: &str) -> Self {
		match str {
			"blockstates" => Self::BlockState,
			"models" => Self::Model,
			"textures" => Self::Texture,
			_ => panic!("Unknown resource kind `{str}`"),
		}
	}
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(from = "String")]
pub struct ResourceLocation {
	pub modid: IString,
	pub name: IString,
}

impl ResourceLocation {
	pub fn new(modid: &str, name: &str) -> Self {
		Self {
			modid: IString::lowercased(modid),
			name: IString::lowercased(name),
		}
	}

	pub fn from_path(path: &Path) -> (Self, ResourceKind) {
		// assets/{modid}/{kind}/{path}
		let modid = path
			.components()
			.skip(1)
			.take(1)
			.next()
			.unwrap()
			.as_os_str()
			.to_str()
			.unwrap();
		let kind = path
			.components()
			.skip(2)
			.take(1)
			.next()
			.unwrap()
			.as_os_str()
			.to_str()
			.unwrap()
			.into();
		let path = path
			.components()
			.skip(3)
			.collect::<PathBuf>()
			.with_extension("")
			.to_str()
			.unwrap()
			.replace(std::path::MAIN_SEPARATOR, "/");
		(Self::new(modid, &path), kind)
	}

	pub fn into_path(self, kind: ResourceKind) -> PathBuf {
		let Self { modid, name } = self;
		let (prefix, extension) = (kind.path_prefix(), kind.extension());
		format!("assets/{modid}/{prefix}/{name}.{extension}").into()
	}
}

impl From<&str> for ResourceLocation {
	fn from(combined: &str) -> Self {
		if let Some((modid, name)) = combined.split_once(":") {
			Self::new(modid, name)
		} else {
			Self {
				modid: IString::from_static("minecraft"),
				name: IString::lowercased(combined),
			}
		}
	}
}

impl From<String> for ResourceLocation {
	fn from(s: String) -> Self {
		s.as_str().into()
	}
}

impl From<ResourceLocation> for String {
	fn from(loc: ResourceLocation) -> Self {
		format!("{}", loc)
	}
}

impl Display for ResourceLocation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("{}:{}", self.modid, self.name))
	}
}

impl Debug for ResourceLocation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(self, f)
	}
}

#[cfg(none)]
impl<'de> Deserialize<'de> for ResourceLocation {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let vis = StringVisitor::new();
		Ok(ResourceLocation::from(
			deserializer.deserialize_str(vis)?.as_str(),
		))
	}
}
