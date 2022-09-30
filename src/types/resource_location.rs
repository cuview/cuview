use std::{fmt::{Display, Debug}, path::{PathBuf, Path}, cell::RefCell};

use super::IString;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
	BlockState,
	Model,
	Texture,
}

impl ResourceKind {
	pub fn path_prefix(self) -> &'static str {
		match self {
			Self::BlockState => "blockstates",
			Self::Model => "models",
			Self::Texture => "textures",
		}
	}
	
	pub fn extension(self) -> &'static str {
		match self {
			Self::BlockState => "json",
			Self::Model => "json",
			Self::Texture => "png",
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceLocation {
	pub modid: IString,
	pub name: IString,
}

impl ResourceLocation {
	pub fn new(modid: &str, name: &str) -> Self {
		Self {
			modid: to_lowercase_istring(modid),
			name: to_lowercase_istring(name),
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
		let ResourceLocation { modid, name } = self;
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
				name: to_lowercase_istring(combined),
			}
		}
	}
}

impl From<ResourceLocation> for String {
    fn from(loc: ResourceLocation) -> Self {
        format!("{}", loc)
    }
}

impl Display for ResourceLocation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("{}:{}", self.modid, self.name))
	}
}

impl Debug for ResourceLocation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		<Self as Display>::fmt(self, f)
	}
}

thread_local! {
	static lowercaseBuffer: RefCell<String> = RefCell::new(String::new());
}

fn to_lowercase_istring(str: &str) -> IString {
	lowercaseBuffer.with(|cell| {
		let mut buffer = cell.borrow_mut();
		buffer.clear();
		buffer.push_str(str);
		buffer.make_ascii_lowercase();
		buffer.as_str().into()
	})
}
