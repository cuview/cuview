#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceLocation {
	modid: String,
	name: String,
}

impl ResourceLocation {
	pub fn new(modid: &str, name: &str) -> Self {
		Self {
			modid: modid.into(),
			name: name.into(),
		}
	}

	pub fn from_str(combined: &str) -> Self {
		if let Some((modid, name)) = combined.split_once(":") {
			Self::new(modid, name)
		} else {
			Self::new("minecraft".into(), combined)
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockPos {
	x: i32,
	y: i32,
	z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkPos {
	x: i32,
	z: i32,
}

impl From<BlockPos> for ChunkPos {
	fn from(pos: BlockPos) -> Self {
		Self {
			x: pos.x >> 4,
			z: pos.z >> 4,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegionPos {
	x: i32,
	z: i32,
}

impl From<BlockPos> for RegionPos {
	fn from(pos: BlockPos) -> Self {
		Self {
			x: pos.x >> 9,
			z: pos.z >> 9,
		}
	}
}

impl From<ChunkPos> for RegionPos {
	fn from(pos: ChunkPos) -> Self {
		Self {
			x: pos.x >> 5,
			z: pos.z >> 5,
		}
	}
}
