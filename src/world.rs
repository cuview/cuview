use std::collections::HashMap;

use crate::types::{ChunkPos, RegionPos, ResourceLocation};

struct World {
	regions: HashMap<RegionPos, Region>,
}

impl World {
	pub fn new() -> Self {
		Self {
			regions: HashMap::new(),
		}
	}

	pub fn new_region(&mut self, pos: RegionPos) -> &mut Region {
		debug_assert!(
			!self.regions.contains_key(&pos),
			"Duplicate region: {:?}",
			pos
		);
		let new = Region::new(pos);
		self.regions.insert(pos, new);
		self.regions.get_mut(&pos).unwrap()
	}

	pub fn get_region(&self, pos: RegionPos) -> Option<&Region> {
		self.regions.get(&pos)
	}

	pub fn get_region_mut(&mut self, pos: RegionPos) -> Option<&mut Region> {
		self.regions.get_mut(&pos)
	}

	pub fn is_region_loaded(&self, pos: RegionPos) -> bool {
		self.regions.contains_key(&pos)
	}
}

struct Region {
	pos: RegionPos,
	chunks: HashMap<ChunkPos, Chunk>,
}

impl Region {
	pub fn new(pos: RegionPos) -> Self {
		Self {
			pos,
			chunks: HashMap::new(),
		}
	}

	pub fn new_chunk(&mut self, pos: ChunkPos) -> &mut Chunk {
		debug_assert!(
			!self.chunks.contains_key(&pos),
			"Duplicate chunk in region {:?}: {:?}",
			self.pos,
			pos
		);
		let new = Chunk::new(pos);
		self.chunks.insert(pos, new);
		self.chunks.get_mut(&pos).unwrap()
	}

	pub fn get_chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
		self.chunks.get(&pos)
	}

	pub fn get_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
		self.chunks.get_mut(&pos)
	}
}

struct Chunk {
	pos: ChunkPos,
	sections: HashMap<u8, ChunkSection>,
}

impl Chunk {
	pub fn new(pos: ChunkPos) -> Self {
		Self {
			pos,
			sections: HashMap::new(),
		}
	}

	pub fn new_section(&mut self, y: u8) -> &mut ChunkSection {
		debug_assert!(
			!self.sections.contains_key(&y),
			"Duplicate chunk section in {:?}: {:?}",
			self.pos,
			y
		);
		let new = ChunkSection::new(y);
		self.sections.insert(y, new);
		self.sections.get_mut(&y).unwrap()
	}

	pub fn get_section(&self, y: u8) -> Option<&ChunkSection> {
		self.sections.get(&y)
	}

	pub fn get_section_mut(&mut self, y: u8) -> Option<&mut ChunkSection> {
		self.sections.get_mut(&y)
	}
}

struct ChunkSection {
	y: u8,
	palette: Palette,
	blocks: Vec<u32>,
}

impl ChunkSection {
	pub fn new(y: u8) -> Self {
		let mut blocks = Vec::new();
		blocks.resize(16usize.pow(3), u32::MAX);
		Self {
			y,
			palette: Palette::new(),
			blocks,
		}
	}
}

struct Palette {
	map: HashMap<u32, ResourceLocation>,
}

impl Palette {
	pub fn new() -> Self {
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
