use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::{self, Debug, Display, Write};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

use super::{IString, ResourceLocation};
use crate::loader::blockstate::{BlockStates, State};
use crate::world::Palette;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockState {
	block: ResourceLocation,
	props: IString,
}

impl BlockState {
	pub fn stateless(block: ResourceLocation) -> Self {
		Self {
			block,
			props: IString::from_static(""),
		}
	}

	pub fn from_multipart_model() -> Self {
		todo!()
	}

	pub fn block_name(&self) -> ResourceLocation {
		self.block
	}

	pub fn get_property(&self, key: &str) -> Option<&str> {
		for pair in self.props.split(",") {
			let mut split = pair.splitn(2, "=");
			let (k, v) = (split.next()?, split.next()?);
			if key == k {
				return Some(v);
			}
		}
		None
	}
}

impl Display for BlockState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("{}[{}]", self.block, self.props))
	}
}

impl Debug for BlockState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(self, f)
	}
}

#[test]
fn test_blockstate() {
	let block = "test".into();
	let s1 = BlockState::stateless(block);
	let s2 = BlockState::stateless(block);
	assert!(s1 == s2);
	assert!(s1.get_property("abc") == None);

	let state = BlockState {
		block,
		props: "abc=1".into(),
	};
	assert!(state.get_property("abc") == Some("1"));

	let state = BlockState {
		block,
		props: "abc=1,def=2".into(),
	};
	assert!(state.get_property("abc") == Some("1"));
	assert!(state.get_property("def") == Some("2"));
}

pub struct BlockStateBuilder {
	block: ResourceLocation,
	props: BTreeMap<IString, IString>,
}

impl BlockStateBuilder {
	pub fn new(block: ResourceLocation) -> Self {
		Self {
			block,
			props: BTreeMap::new(),
		}
	}

	pub fn from_variants_model(block: ResourceLocation, props: &str) -> Self {
		assert!(props.len() > 0);
		let mut this = Self::new(block);
		for prop in props.split(",") {
			let mut split = prop.splitn(2, "=");
			let key = split.next().unwrap();
			let value = split.next().unwrap();
			this.set_property(key, value);
		}
		this
	}

	pub fn build(self) -> BlockState {
		let mut props = String::with_capacity(256);
		for (i, (k, v)) in self.props.into_iter().enumerate() {
			let comma = if i == 0 { "" } else { "," };
			props.write_fmt(format_args!("{comma}{k}={v}")).unwrap();
		}
		BlockState {
			block: self.block,
			props: props.into(),
		}
	}

	pub fn keys(&self) -> impl '_ + Iterator<Item = &'static str> {
		self.props.keys().map(IString::as_str)
	}

	pub fn get_property(&self, key: &str) -> Option<&'static str> {
		self.props
			.get(&IString::lowercased(key))
			.map(IString::as_str)
	}

	pub fn set_property(&mut self, key: &str, value: &str) {
		self.props
			.insert(IString::lowercased(key), IString::lowercased(value));
	}
}

#[test]
fn test_builder() {
	let block = "test".into();
	let mut builder = BlockStateBuilder::new(block);
	assert!(builder.get_property("abc") == None);

	builder.set_property("def", "1");
	assert!(builder.get_property("def") == Some("1"));
	builder.set_property("abc", "2");
	assert!(builder.get_property("abc") == Some("2"));

	let state = builder.build();
	assert!(state.props.as_str() == "abc=2,def=1");
}

#[derive(Clone, Debug)]
pub struct BlockStateCache(HashMap<ResourceLocation, Vec<BlockState>>);

impl BlockStateCache {
	pub fn new() -> Self {
		Self(HashMap::new())
	}

	pub fn from_json(states: BlockStates) -> Self {
		let mut res = HashMap::with_capacity(states.0.len());
		for (block, def) in states.0 {
			assert!(res.insert(block, vec![]).is_none());

			for state in def.states {
				let State {
					properties,
					default,
					..
				} = state;
				let mut builder = BlockStateBuilder::new(block);
				if let Some(properties) = properties {
					for (k, v) in properties {
						builder.set_property(&k, &v);
					}
				}

				let state = builder.build();
				let vec = res.get_mut(&block).unwrap();
				if default {
					vec.insert(0, state);
				} else {
					vec.push(state);
				}
			}
		}
		Self(res)
	}

	pub fn blocks(&self) -> impl '_ + Iterator<Item = ResourceLocation> {
		self.0.keys().copied()
	}

	pub fn states(&self) -> impl '_ + Iterator<Item = BlockState> {
		self.0.values().flat_map(Vec::as_slice).copied()
	}

	pub fn states_of(&self, block: ResourceLocation) -> Option<&[BlockState]> {
		self.0.get(&block).map(Vec::as_slice)
	}

	pub fn default_state_of(&self, block: ResourceLocation) -> Option<BlockState> {
		self.0.get(&block).and_then(|xs| xs.first()).copied()
	}
}
