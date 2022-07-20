use std::borrow::Borrow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::{ChunkPos, RegionPos, ResourceLocation};
use crate::{clone_shared, make_shared, Shared};

pub struct World {
	rootDir: PathBuf,
	dimensions: HashMap<ResourceLocation, Dimension>,
	pub spawnpoint: (i32, i32, i32),
}

impl World {
	pub fn new(rootDir: impl AsRef<Path>) -> Shared<Self> {
		make_shared(Self {
			rootDir: rootDir.as_ref().into(),
			dimensions: HashMap::new(),
			spawnpoint: Default::default(),
		})
	}

	pub fn rootDir(&self) -> &Path {
		self.rootDir.borrow()
	}

	// pub fn
}

pub struct Dimension {
	id: ResourceLocation,
	regions: HashMap<RegionPos, Shared<Region>>,
}

impl Dimension {
	fn new(id: ResourceLocation) -> Shared<Self> {
		make_shared(Self {
			id,
			regions: HashMap::new(),
		})
	}

	pub fn new_region(&mut self, pos: RegionPos) -> Shared<Region> {
		debug_assert!(
			!self.regions.contains_key(&pos),
			"Duplicate region: {:?}",
			pos
		);
		let new = Region::new(pos);
		self.regions.insert(pos, new);
		clone_shared(self.regions.get(&pos).unwrap())
	}

	pub fn get_region(&self, pos: RegionPos) -> Option<Shared<Region>> {
		self.regions.get(&pos).map(clone_shared)
	}

	pub fn is_region_loaded(&self, pos: RegionPos) -> bool {
		self.regions.contains_key(&pos)
	}
}

pub struct Region {
	pos: RegionPos,
	chunks: HashMap<ChunkPos, Shared<Chunk>>,
}

impl Region {
	fn new(pos: RegionPos) -> Shared<Self> {
		make_shared(Self {
			pos,
			chunks: HashMap::new(),
		})
	}

	pub fn new_chunk(&mut self, pos: ChunkPos) -> Shared<Chunk> {
		debug_assert!(
			!self.chunks.contains_key(&pos),
			"Duplicate chunk in region {:?}: {:?}",
			self.pos,
			pos
		);
		let new = Chunk::new(pos);
		self.chunks.insert(pos, new);
		clone_shared(self.chunks.get_mut(&pos).unwrap())
	}

	pub fn get_chunk(&self, pos: ChunkPos) -> Option<Shared<Chunk>> {
		self.chunks.get(&pos).map(clone_shared)
	}
}

pub struct Chunk {
	pos: ChunkPos,
	sections: HashMap<u8, Shared<ChunkSection>>,
}

impl Chunk {
	fn new(pos: ChunkPos) -> Shared<Self> {
		make_shared(Self {
			pos,
			sections: HashMap::new(),
		})
	}

	pub fn new_section(&mut self, y: u8) -> Shared<ChunkSection> {
		debug_assert!(
			!self.sections.contains_key(&y),
			"Duplicate chunk section in {:?}: {:?}",
			self.pos,
			y
		);
		let new = ChunkSection::new(y);
		self.sections.insert(y, new);
		clone_shared(self.sections.get_mut(&y).unwrap())
	}

	pub fn get_section(&self, y: u8) -> Option<Shared<ChunkSection>> {
		self.sections.get(&y).map(clone_shared)
	}
}

pub struct ChunkSection {
	y: u8,
	palette: Palette,
	blocks: Vec<u32>,
}

impl ChunkSection {
	fn new(y: u8) -> Shared<Self> {
		let mut blocks = Vec::new();
		blocks.resize(16usize.pow(3), u32::MAX);
		make_shared(Self {
			y,
			palette: Palette::new(),
			blocks,
		})
	}
}

pub struct Palette {
	map: HashMap<u32, ResourceLocation>,
}

impl Palette {
	fn new() -> Self {
		Self {
			map: HashMap::new(),
		}
	}

	pub fn lookup_id(&self, id: u32) -> Option<&ResourceLocation> {
		self.map.get(&id)
	}

	pub fn lookup_block(&self, block: &ResourceLocation) -> Option<u32> {
		self.map.iter().find(|(_, v)| *v == block).map(|(k, _)| *k)
	}

	pub fn define(&mut self, id: u32, block: &ResourceLocation) {
		if cfg!(debug_assertions) {
			let oldId = self.lookup_block(block);
			assert!(
				oldId.is_none(),
				"Duplicate block {:?} in palette with ids {}/{}",
				block,
				oldId.unwrap(),
				id
			);
		}
		self.map.insert(id, block.clone());
	}
}
