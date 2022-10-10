use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Shr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::{fmt, io};

use crate::loader::common::AnvilRegion;
use crate::types::blockstate::{BlockState, BlockStateBuilder};
use crate::types::coords::{ChunkPos, RegionPos};
use crate::types::shared::{Shared, WeakShared};
use crate::types::ResourceLocation;

pub struct World {
	this: WeakShared<Self>,
	rootDir: PathBuf,
	dimensions: HashMap<ResourceLocation, Shared<Dimension>>,
}

impl World {
	pub fn new(rootDir: impl AsRef<Path>) -> Shared<Self> {
		Shared::new_cyclic(|this| Self {
			this: this.clone(),
			rootDir: rootDir.as_ref().into(),
			dimensions: HashMap::new(),
		})
	}

	pub fn root_dir(&self) -> &Path {
		self.rootDir.borrow()
	}

	pub fn new_dimension(
		&mut self,
		id: ResourceLocation,
		dimensionRoot: &Path,
	) -> Shared<Dimension> {
		debug_assert!(
			!self.dimensions.contains_key(&id),
			"Duplicate dimension {:?}",
			id
		);
		let this = self.this.upgrade().expect("null this");
		let new = Dimension::new(this, id, dimensionRoot);
		self.dimensions.insert(id, new.clone());
		new
	}

	pub fn unload_dimension(&mut self, id: ResourceLocation) {
		self.dimensions.remove(&id);
	}
}

impl Debug for World {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("World")
			.field("rootDir", &self.rootDir)
			.field("dimensions", &self.dimensions)
			.finish()
	}
}

pub struct Dimension {
	this: WeakShared<Self>,
	world: Shared<World>,
	id: ResourceLocation,
	rootDir: PathBuf,
	regions: HashMap<RegionPos, Shared<Region>>,
}

impl Dimension {
	fn new(world: Shared<World>, id: ResourceLocation, rootDir: &Path) -> Shared<Self> {
		Shared::new_cyclic(|this| Self {
			this: this.clone(),
			world,
			id,
			rootDir: rootDir.into(),
			regions: HashMap::new(),
		})
	}

	pub fn id(&self) -> ResourceLocation {
		self.id
	}

	pub fn root_dir(&self) -> &Path {
		&self.rootDir
	}

	pub fn region_dir(&self) -> PathBuf {
		self.rootDir.join("region")
	}

	pub fn world(&self) -> Shared<World> {
		self.world.clone()
	}

	pub fn new_region(&mut self, pos: RegionPos) -> Shared<Region> {
		debug_assert!(
			!self.regions.contains_key(&pos),
			"Duplicate region {:?}",
			pos
		);
		let this = self.this.upgrade().expect("null this");
		let new = Region::new(this, pos);
		self.regions.insert(pos, new.clone());
		new
	}

	pub fn unload_region(&mut self, pos: RegionPos) {
		self.regions.remove(&pos);
	}

	pub fn get_region(&self, pos: RegionPos) -> Option<Shared<Region>> {
		self.regions.get(&pos).map(Shared::clone)
	}

	pub fn is_region_loaded(&self, pos: RegionPos) -> bool {
		self.regions.contains_key(&pos)
	}
}

impl Debug for Dimension {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Dimension")
			.field("world", &self.world.borrow().root_dir())
			.field("id", &self.id)
			.field("rootDir", &self.rootDir)
			.field("regions", &self.regions)
			.finish()
	}
}

pub struct Region {
	this: WeakShared<Self>,
	dimension: Shared<Dimension>,
	pos: RegionPos,
	anvil: Option<Arc<AnvilRegion>>, // not `Shared` as it doesn't need mutability
	chunks: HashMap<ChunkPos, Shared<Chunk>>,
}

impl Region {
	fn new(dimension: Shared<Dimension>, pos: RegionPos) -> Shared<Self> {
		Shared::new_cyclic(|this| Self {
			this: this.clone(),
			dimension,
			pos,
			anvil: None,
			chunks: HashMap::new(),
		})
	}

	pub fn pos(&self) -> RegionPos {
		self.pos
	}

	pub fn world(&self) -> Shared<World> {
		self.dimension.borrow().world.clone()
	}

	pub fn dimension(&self) -> Shared<Dimension> {
		self.dimension.clone()
	}

	pub fn anvil(&self) -> Option<Arc<AnvilRegion>> {
		self.anvil.as_ref().map(|v| Arc::clone(v))
	}

	pub fn load_anvil<'a>(&'a mut self) -> Result<Arc<AnvilRegion>, io::Error> {
		self.anvil = Some(AnvilRegion::new(self.dimension.borrow().region_dir(), self.pos)?.into());
		self.anvil()
			.ok_or_else(|| unreachable!("self.anvil cannot be None"))
	}

	pub fn new_chunk(&mut self, pos: ChunkPos) -> Shared<Chunk> {
		debug_assert!(
			!self.chunks.contains_key(&pos),
			"Duplicate chunk {:?} (region {:?})",
			self.pos,
			pos
		);
		let this = self.this.upgrade().expect("null this");
		let new = Chunk::new(this, pos);
		self.chunks.insert(pos, new.clone());
		new
	}

	pub fn get_chunk(&self, pos: ChunkPos) -> Option<Shared<Chunk>> {
		self.chunks.get(&pos).map(Shared::clone)
	}
}

impl Debug for Region {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Region")
			.field("dimension", &self.dimension.borrow().id())
			.field("pos", &self.pos)
			.field("chunks", &self.chunks)
			.finish()
	}
}

pub struct Chunk {
	this: WeakShared<Self>,
	region: Shared<Region>,
	pos: ChunkPos,
	sections: HashMap<i8, Shared<ChunkSection>>,
}

impl Chunk {
	fn new(region: Shared<Region>, pos: ChunkPos) -> Shared<Self> {
		Shared::new_cyclic(|this| Self {
			this: this.clone(),
			region,
			pos,
			sections: HashMap::new(),
		})
	}

	pub fn pos(&self) -> ChunkPos {
		self.pos
	}

	pub fn world(&self) -> Shared<World> {
		self.region.borrow().dimension.borrow().world.clone()
	}

	pub fn dimension(&self) -> Shared<Dimension> {
		self.region.borrow().dimension.clone()
	}

	pub fn region(&self) -> Shared<Region> {
		self.region.clone()
	}

	pub fn new_section(&mut self, y: i8) -> Shared<ChunkSection> {
		debug_assert!(
			!self.sections.contains_key(&y),
			"Duplicate chunk section in {:?}: {:?}",
			self.pos,
			y
		);
		let this = self.this.upgrade().expect("null this");
		let new = ChunkSection::new(this, y);
		self.sections.insert(y, new.clone());
		new
	}

	pub fn get_section(&self, y: i8) -> Option<Shared<ChunkSection>> {
		self.sections.get(&y).map(Shared::clone)
	}
}

impl Debug for Chunk {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Chunk")
			.field("region", &self.region.borrow().pos())
			.field("pos", &self.pos)
			.field("sections", &self.sections)
			.finish()
	}
}

pub struct ChunkSection {
	chunk: Shared<Chunk>,
	palette: Shared<Palette>,
	blocks: Vec<u32>,
	y: i8,
}

impl ChunkSection {
	fn new(chunk: Shared<Chunk>, y: i8) -> Shared<Self> {
		let mut blocks = Vec::new();
		blocks.resize(16usize.pow(3), u32::MAX);
		Self {
			chunk,
			palette: Shared::new(Palette::new()), // TODO `Option<Palette>` to fall back to global?
			blocks,
			y,
		}
		.into()
	}

	pub fn pos(&self) -> (ChunkPos, i8) {
		(self.chunk.borrow().pos, self.y)
	}

	pub fn world(&self) -> Shared<World> {
		self.chunk
			.borrow()
			.region
			.borrow()
			.dimension
			.borrow()
			.world
			.clone()
	}

	pub fn dimension(&self) -> Shared<Dimension> {
		self.chunk.borrow().region.borrow().dimension.clone()
	}

	pub fn region(&self) -> Shared<Region> {
		self.chunk.borrow().region.clone()
	}

	pub fn chunk(&self) -> Shared<Chunk> {
		self.chunk.clone()
	}
}

impl Debug for ChunkSection {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ChunkSection")
			.field("chunk", &self.chunk)
			.field("palette", &self.palette)
			.field("blocks", &self.blocks)
			.field("y", &self.y)
			.finish()
	}
}

#[derive(Clone)]
pub struct Palette {
	idToLoc: HashMap<u32, BlockState>,
	locToId: HashMap<BlockState, u32>,
}

impl Palette {
	pub fn new() -> Self {
		Self {
			idToLoc: HashMap::new(),
			locToId: HashMap::new(),
		}
	}

	pub fn define(&mut self, id: u32, state: BlockState) {
		let oldState = self.get_state(id);
		assert!(
			oldState.is_none(),
			"Duplicate states {:?}/{:?} in palette with id {}",
			oldState.unwrap(),
			state,
			id
		);

		let oldId = self.get_id(state);
		assert!(
			oldId.is_none(),
			"Duplicate block {:?} in palette with ids {}/{}",
			state,
			oldId.unwrap(),
			id
		);

		self.idToLoc.insert(id, state);
		self.locToId.insert(state, id);
	}

	pub fn get_state(&self, id: u32) -> Option<BlockState> {
		self.idToLoc.get(&id).map(|v| *v)
	}

	pub fn get_id(&self, block: BlockState) -> Option<u32> {
		self.locToId.get(&block).map(|v| *v)
	}

	pub fn bits(&self) -> usize {
		let maxId = match self.idToLoc.keys().max() {
			None => return 0,
			Some(&v) if v < 16 => return 4,
			Some(&v) => v,
		};

		let add = if maxId.count_ones() == 1 { 1 } else { 0 };
		(maxId.next_power_of_two().trailing_zeros() + add) as usize
	}
}

impl Debug for Palette {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// print entries in order of id
		let entries: std::collections::BTreeMap<_, _> = self.idToLoc.iter().collect();
		f.debug_struct("Palette")
			.field("entries", &entries)
			.finish()
	}
}

#[test]
fn test_palette() {
	let mut p = Palette::new();
	assert!(p.bits() == 0);

	let nil = BlockState::stateless("nil".into());
	let air = BlockState::stateless("air".into());
	p.define(0, air);
	assert!(p.get_id(air).unwrap_or(u32::MAX) == 0);
	assert!(p.get_state(0).unwrap_or(nil) == air);
	assert!(p.bits() == 4);

	for i in 1 ..= 16 {
		p.define(i, BlockState::stateless(i.to_string().as_str().into()));
	}
	assert!(p.bits() == 5);

	// TODO: registry overrides/aliases/etc.
	use std::panic::catch_unwind;
	{
		let mut p = p.clone();
		catch_unwind(move || p.define(0, nil)).unwrap_err();
	}
	{
		let mut p = p.clone();
		catch_unwind(move || p.define(64, air)).unwrap_err();
	}
}
