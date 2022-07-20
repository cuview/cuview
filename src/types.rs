#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceLocation {
	pub modid: String,
	pub name: String,
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
	pub x: i32,
	pub y: i32,
	pub z: i32,
}

impl BlockPos {
	pub fn new(x: i32, y: i32, z: i32) -> Self {
		Self { x, y, z }
	}

	pub fn chunk_relative(&self) -> Self {
		Self {
			x: self.x.rem_euclid(16),
			y: self.y.rem_euclid(16),
			z: self.z.rem_euclid(16),
		}
	}
}

#[test]
fn test_blockpos() {
	let pos = BlockPos::new(0, 0, 0);
	assert!(pos.chunk_relative() == pos);

	let pos = BlockPos::new(-1, 0, 0);
	assert!(pos.chunk_relative() == BlockPos::new(15, 0, 0));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkPos {
	pub x: i32,
	pub z: i32,
}

impl ChunkPos {
	pub fn new(x: i32, z: i32) -> Self {
		Self { x, z }
	}

	pub fn region_relative(&self) -> Self {
		Self {
			x: self.x.rem_euclid(32),
			z: self.z.rem_euclid(32),
		}
	}
}

impl From<BlockPos> for ChunkPos {
	fn from(pos: BlockPos) -> Self {
		Self {
			x: pos.x >> 4,
			z: pos.z >> 4,
		}
	}
}

#[test]
fn test_chunkpos() {
	let pos = ChunkPos::new(0, 0);
	assert!(pos.region_relative() == pos);

	let pos = ChunkPos::new(-1, 0);
	assert!(pos.region_relative() == ChunkPos::new(31, 0));

	assert!(ChunkPos::from(BlockPos::new(-1, 0, 0)) == pos);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegionPos {
	pub x: i32,
	pub z: i32,
}

impl RegionPos {
	pub fn new(x: i32, z: i32) -> Self {
		Self { x, z }
	}
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
