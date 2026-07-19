use std::{
	borrow::Borrow,
	collections::HashMap,
	fmt,
	fmt::Debug,
	io,
	ops::Range,
	path::{Path, PathBuf},
	sync::{Arc, Weak},
};

use crate::{
	loader::common::AnvilRegion,
	types::{
		BlockPos,
		ResourceLocation,
		blockstate::{BlockState, BlockStateBuilder},
		coords::{ChunkPos, RegionPos},
		shared::{Shared, WeakShared},
	},
};

pub struct World {
	this: WeakShared<Self>,
	root_dir: PathBuf,
	dimensions: HashMap<ResourceLocation, Shared<Dimension>>,
}

impl World {
	pub fn new(root_dir: impl AsRef<Path>) -> Shared<Self> {
		Shared::new_cyclic(|this| Self {
			this: this.clone(),
			root_dir: root_dir.as_ref().into(),
			dimensions: HashMap::new(),
		})
	}

	pub fn root_dir(&self) -> &Path {
		self.root_dir.borrow()
	}

	pub fn new_dimension(
		&mut self,
		id: ResourceLocation,
		dimension_root: &Path,
	) -> Shared<Dimension> {
		debug_assert!(
			!self.dimensions.contains_key(&id),
			"Duplicate dimension {:?}",
			id
		);
		let this = self.this.upgrade().expect("null this");
		let new = Dimension::new(this, id, dimension_root);
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
			.field("rootDir", &self.root_dir)
			.field("dimensions", &self.dimensions)
			.finish()
	}
}

pub struct Dimension {
	this: WeakShared<Self>,
	world: Shared<World>,
	id: ResourceLocation,
	root_dir: PathBuf,
	regions: HashMap<RegionPos, Shared<Region>>,
}

impl Dimension {
	fn new(world: Shared<World>, id: ResourceLocation, root_dir: &Path) -> Shared<Self> {
		Shared::new_cyclic(|this| Self {
			this: this.clone(),
			world,
			id,
			root_dir: root_dir.into(),
			regions: HashMap::new(),
		})
	}

	pub fn id(&self) -> ResourceLocation {
		self.id
	}

	pub fn root_dir(&self) -> &Path {
		&self.root_dir
	}

	pub fn region_dir(&self) -> PathBuf {
		self.root_dir.join("region")
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
		let new = Region::new(this, pos, &self.region_dir());
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
			.field("rootDir", &self.root_dir)
			.field("regions", &self.regions)
			.finish()
	}
}

pub struct Region {
	this: WeakShared<Self>,
	dimension: Shared<Dimension>,
	pos: RegionPos,
	anvil: Arc<AnvilRegion>, // not `Shared` as it doesn't need mutability
	chunks: HashMap<ChunkPos, Shared<Chunk>>,
}

impl Region {
	fn new(dimension: Shared<Dimension>, pos: RegionPos, region_dir: &Path) -> Shared<Self> {
		let anvil = AnvilRegion::new(region_dir, pos).unwrap().into();
		Shared::new_cyclic(|this| Self {
			this: this.clone(),
			dimension,
			pos,
			anvil,
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

	pub fn anvil(&self) -> Arc<AnvilRegion> {
		Arc::clone(&self.anvil)
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

	pub fn new_section(&mut self, y: i8, palette: Palette) -> Shared<ChunkSection> {
		debug_assert!(
			!self.sections.contains_key(&y),
			"Duplicate chunk section in {:?}: {:?}",
			self.pos,
			y
		);
		let this = self.this.upgrade().expect("null this");
		let new = ChunkSection::new(this, self.pos, y, palette);
		self.sections.insert(y, new.clone());
		new
	}

	pub fn get_section(&self, y: i8) -> Option<Shared<ChunkSection>> {
		self.sections.get(&y).map(Shared::clone)
	}

	pub fn sections(&self) -> Range<i8> {
		let min = self.sections.keys().copied().min().unwrap_or(0);
		let max = self
			.sections
			.keys()
			.copied()
			.max()
			.map(|v| v + 1)
			.unwrap_or(0);
		min .. max
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
	pos: ChunkPos,
	y: i8,
	palette: Shared<Palette>,
	blocks: Vec<u32>,
}

impl ChunkSection {
	fn new(chunk: Shared<Chunk>, pos: ChunkPos, y: i8, palette: Palette) -> Shared<Self> {
		let mut blocks = Vec::new();
		blocks.resize(16usize.pow(3), u32::MAX);
		Self {
			chunk,
			pos,
			y,
			palette: Shared::new(palette),
			blocks,
		}
		.into()
	}

	pub fn pos(&self) -> (ChunkPos, i8) {
		(self.pos, self.y)
	}

	pub fn palette(&self) -> Shared<Palette> {
		self.palette.clone()
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

	fn index_of(&self, pos: BlockPos) -> usize {
		debug_assert_eq!(ChunkPos::from(pos), self.pos);
		debug_assert_eq!(pos.section(), self.y);
		let pos = pos.chunk_relative();
		((pos.y * ChunkPos::DIAMETER_BLOCKS.pow(2)) + (pos.z * ChunkPos::DIAMETER_BLOCKS) + pos.x)
			as usize
	}

	pub fn get_block(&self, pos: BlockPos) -> BlockState {
		let id = self.blocks[self.index_of(pos)];
		let palette = self.palette.borrow();
		palette
			.get_state(id)
			.unwrap_or_else(|| panic!("{pos:?} {id} {palette:?}"))
	}

	pub fn set_block(&mut self, pos: BlockPos, state: BlockState) {
		let index = self.index_of(pos);
		self.blocks[index] = self.palette.borrow().get_id(state).unwrap();
	}

	pub fn fill_with_block(&mut self, state: BlockState) {
		let id = self.palette.borrow().get_id(state).unwrap();
		self.blocks.fill(id);
	}

	pub fn fill_from_iter(&mut self, paletted_blocks: impl Iterator<Item = u32>) {
		let mut len = 0;
		for (pos, id) in self.pos.blocks_in_section(self.y).zip(paletted_blocks) {
			len += 1;
			let index = self.index_of(pos);
			self.blocks[index] = id;
		}
		debug_assert_eq!(len, 4096);
	}
}

impl Debug for ChunkSection {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ChunkSection")
			.field("pos", &self.pos)
			.field("y", &self.y)
			.field("palette", &self.palette)
			.finish()
	}
}

#[derive(Clone)]
pub struct Palette {
	id_to_loc: HashMap<u32, BlockState>,
	loc_to_id: HashMap<BlockState, u32>,
}

impl Palette {
	pub fn new() -> Self {
		Self {
			id_to_loc: HashMap::new(),
			loc_to_id: HashMap::new(),
		}
	}

	pub fn define(&mut self, id: u32, state: BlockState) {
		let old_state = self.get_state(id);
		assert!(
			old_state.is_none(),
			"Duplicate states {:?}/{:?} in palette with id {}",
			old_state.unwrap(),
			state,
			id
		);

		let old_id = self.get_id(state);
		assert!(
			old_id.is_none(),
			"Duplicate block {:?} in palette with ids {}/{}",
			state,
			old_id.unwrap(),
			id
		);

		self.id_to_loc.insert(id, state);
		self.loc_to_id.insert(state, id);
	}

	pub fn get_state(&self, id: u32) -> Option<BlockState> {
		self.id_to_loc.get(&id).map(|v| *v)
	}

	pub fn get_id(&self, block: BlockState) -> Option<u32> {
		self.loc_to_id.get(&block).map(|v| *v)
	}

	pub fn bits(&self) -> usize {
		let max_id = match self.id_to_loc.keys().max() {
			None => return 0,
			Some(&v) if v < 16 => return 4,
			Some(&v) => v,
		};

		let add = if max_id.count_ones() == 1 { 1 } else { 0 };
		(max_id.next_power_of_two().trailing_zeros() + add) as usize
	}
}

impl Debug for Palette {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// print entries in order of id
		let entries: std::collections::BTreeMap<_, _> = self.id_to_loc.iter().collect();
		f.debug_struct("Palette")
			.field("entries", &entries)
			.finish()
	}
}

impl FromIterator<BlockState> for Palette {
	fn from_iter<T: IntoIterator<Item = BlockState>>(iter: T) -> Self {
		let mut res = Self::new();
		for (id, state) in iter.into_iter().enumerate() {
			res.define(id as u32, state);
		}
		res
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
