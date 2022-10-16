use std::{str::FromStr, num::ParseIntError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockPos {
	pub x: i32,
	pub y: i32,
	pub z: i32,
}

impl BlockPos {
	pub const columnHeight: i32 = 384;
	pub const maxHeight: i32 = 319;
	pub const minHeight: i32 = -64;

	pub fn new(x: i32, y: i32, z: i32) -> Self {
		Self { x, y, z }
	}

	pub fn chunk_relative(&self) -> Self {
		Self {
			x: self.x.rem_euclid(ChunkPos::diameterBlocks),
			y: self.y.rem_euclid(ChunkPos::diameterBlocks),
			z: self.z.rem_euclid(ChunkPos::diameterBlocks),
		}
	}

	pub fn section(&self) -> i8 {
		(self.y >> 4) as i8
	}
}

impl FromStr for BlockPos {
	type Err = ParseIntError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut split = s.splitn(3, ",");
		let x: i32 = split.next().unwrap_or("").trim().parse()?;
		let y: i32 = split.next().unwrap_or("").trim().parse()?;
		let z: i32 = split.next().unwrap_or("").trim().parse()?;
		Ok(Self { x, y, z })
	}
}

#[test]
fn test_blockpos() {
	let pos = BlockPos::new(0, 0, 0);
	assert!(pos.chunk_relative() == pos);

	let pos = BlockPos::new(-1, 0, 0);
	assert!(pos.chunk_relative() == BlockPos::new(15, 0, 0));
	
	let pos = BlockPos::from_str("0,0,0");
	assert!(pos == Ok(BlockPos::new(0, 0, 0)));
	let pos = BlockPos::from_str("-1, -1, -1");
	assert!(pos == Ok(BlockPos::new(-1, -1, -1)));
	let pos = BlockPos::from_str("abc");
	assert!(pos.is_err());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkPos {
	pub x: i32,
	pub z: i32,
}

impl ChunkPos {
	pub const diameterBlocks: i32 = 16;

	pub fn new(x: i32, z: i32) -> Self {
		Self { x, z }
	}

	pub fn region_relative(&self) -> Self {
		Self {
			x: self.x.rem_euclid(RegionPos::diameterChunks),
			z: self.z.rem_euclid(RegionPos::diameterChunks),
		}
	}

	pub fn min_block(&self) -> BlockPos {
		BlockPos::new(
			self.x * Self::diameterBlocks,
			BlockPos::minHeight,
			self.z * Self::diameterBlocks,
		)
	}

	pub fn max_block(&self) -> BlockPos {
		let diameter = Self::diameterBlocks;
		BlockPos::new(
			self.x * diameter + diameter - 1,
			BlockPos::maxHeight,
			self.z * diameter + diameter - 1,
		)
	}

	pub fn blocks(&self) -> impl Iterator<Item = BlockPos> {
		let min = self.min_block();
		let max = self.max_block();
		(min.y ..= max.y).flat_map(move |y| {
			(min.z ..= max.z)
				.flat_map(move |z| (min.x ..= max.x).map(move |x| BlockPos::new(x, y, z)))
		})
	}

	pub fn blocks_in_section(&self, y: i8) -> impl Iterator<Item = BlockPos> + Clone {
		let min = self.min_block();
		let max = self.max_block();
		let minY = y as i32 * 16;
		(minY .. minY + Self::diameterBlocks).flat_map(move |y| {
			(min.z ..= max.z)
				.flat_map(move |z| (min.x ..= max.x).map(move |x| BlockPos::new(x, y, z)))
		})
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

impl FromStr for ChunkPos {
	type Err = ParseIntError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut split = s.splitn(2, ",");
		let x: i32 = split.next().unwrap_or("").trim().parse()?;
		let z: i32 = split.next().unwrap_or("").trim().parse()?;
		Ok(Self { x, z })
	}
}

#[test]
fn test_chunkpos() {
	let pos = ChunkPos::new(0, 0);
	assert!(pos.region_relative() == pos);

	let pos = ChunkPos::new(-1, 0);
	assert!(pos.region_relative() == ChunkPos::new(31, 0));

	assert!(ChunkPos::from(BlockPos::new(-1, 0, 0)) == pos);

	let pos = ChunkPos::new(0, 0);
	assert!(pos.min_block() == BlockPos::new(0, BlockPos::minHeight, 0));
	assert!(pos.max_block() == BlockPos::new(15, BlockPos::maxHeight, 15));
	assert!(
		pos.blocks().count() as i32 == ChunkPos::diameterBlocks.pow(2) * BlockPos::columnHeight
	);

	let pos = ChunkPos::new(-1, -1);
	assert!(pos.min_block() == BlockPos::new(-16, BlockPos::minHeight, -16));
	assert!(pos.max_block() == BlockPos::new(-1, BlockPos::maxHeight, -1));

	let sectionBlocks: Vec<_> = pos.blocks_in_section(0).collect();
	assert!(sectionBlocks.len() as i32 == ChunkPos::diameterBlocks.pow(3));
	assert!(sectionBlocks[0].y == 0);
	assert!(sectionBlocks.last().unwrap().y == 15);

	let sectionBlocks: Vec<_> = pos.blocks_in_section(-1).collect();
	assert!(sectionBlocks.first().unwrap().y == -16);
	assert!(sectionBlocks.last().unwrap().y == -1);
	
	let pos = ChunkPos::from_str("0,0");
	assert!(pos == Ok(ChunkPos::new(0, 0)));
	let pos = ChunkPos::from_str("-1, -1");
	assert!(pos == Ok(ChunkPos::new(-1, -1)));
	let pos = ChunkPos::from_str("abc");
	assert!(pos.is_err());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegionPos {
	pub x: i32,
	pub z: i32,
}

impl RegionPos {
	pub const diameterChunks: i32 = 32;

	pub fn new(x: i32, z: i32) -> Self {
		Self { x, z }
	}

	pub fn min_chunk(&self) -> ChunkPos {
		ChunkPos::new(self.x * Self::diameterChunks, self.z * Self::diameterChunks)
	}

	pub fn max_chunk(&self) -> ChunkPos {
		let diameter = Self::diameterChunks;
		ChunkPos::new(
			self.x * diameter + diameter - 1,
			self.z * diameter + diameter - 1,
		)
	}

	pub fn chunks(&self) -> impl Iterator<Item = ChunkPos> {
		let min = self.min_chunk();
		let max = self.max_chunk();
		(min.z ..= max.z).flat_map(move |z| (min.x ..= max.x).map(move |x| ChunkPos::new(x, z)))
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

impl FromStr for RegionPos {
	type Err = <ChunkPos as FromStr>::Err;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let ChunkPos { x, z } = <ChunkPos as FromStr>::from_str(s)?;
		Ok(Self { x, z })
	}
}

#[test]
fn test_regionpos() {
	let pos = RegionPos::new(0, 0);
	assert!(pos.min_chunk() == ChunkPos::new(0, 0));
	assert!(pos.max_chunk() == ChunkPos::new(31, 31));
	assert!(pos.chunks().count() as i32 == RegionPos::diameterChunks.pow(2));

	let pos = RegionPos::new(-1, -1);
	assert!(pos.min_chunk() == ChunkPos::new(-32, -32));
	assert!(pos.max_chunk() == ChunkPos::new(-1, -1));
}
