use std::collections::hash_map::DefaultHasher;
use std::fmt::{self, Debug};
use std::hash::Hash;

use super::{IString, ResourceLocation};

#[derive(Clone, Copy, Hash)]
pub struct BlockState {
	blockName: ResourceLocation,
	properties: [Option<(IString, IString)>; 16],
}

impl BlockState {
	pub fn new(blockName: ResourceLocation) -> Self {
		Self {
			blockName,
			properties: [None; 16],
		}
	}

	pub fn block_name(&self) -> ResourceLocation {
		self.blockName
	}

	pub fn get_property(&self, key: IString) -> Option<IString> {
		for elem in self.properties {
			match elem {
				Some((k, v)) if k == key => return Some(v),
				_ => {},
			}
		}
		None
	}

	pub fn set_property(&mut self, key: IString, value: IString) {
		// overwrite key if exists
		for prop in self.properties.iter_mut() {
			if let Some((k, _)) = prop {
				if key == *k {
					*prop = Some((key, value));
					return;
				}
			}
		}

		// otherwise add new key
		for prop in self.properties.iter_mut() {
			if prop.is_none() {
				*prop = Some((key, value));
				return;
			}
		}

		assert!(
			false,
			"trying to set more than 16 properties on a blockstate"
		);
	}
}

impl PartialEq for BlockState {
	fn eq(&self, other: &Self) -> bool {
		if self.blockName != other.blockName {
			return false;
		}

		// ensure other has all our properties
		for prop in &self.properties {
			if let Some((k, v)) = prop {
				if let Some(other) = other.get_property(*k) {
					if *v != other {
						return false;
					}
				} else {
					return false;
				}
			}
		}

		// ensure we have all of other's properties
		for prop in &other.properties {
			if let Some((k, other)) = prop {
				if let Some(v) = self.get_property(*k) {
					if *other != v {
						return false;
					}
				} else {
					return false;
				}
			}
		}

		true
	}
}

impl Eq for BlockState {}

impl Debug for BlockState {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("BlockState")
			.field("blockName", &self.blockName)
			.field(
				"properties",
				&self
					.properties
					.iter()
					.filter(|v| v.is_some())
					.map(|v| v.unwrap())
					.collect::<Vec<_>>(),
			)
			.finish()
	}
}

#[test]
fn test_blockstate() {
	let mut state1 = BlockState::new("foo:bar".into());
	let mut state2 = state1.clone();
	assert!(state1 == state2);

	state1.set_property("abc".into(), "one".into());
	state1.set_property("def".into(), "two".into());
	state2.set_property("def".into(), "two".into());
	state2.set_property("abc".into(), "one".into());
	assert!(state1 == state2);
	assert!(state1.hash(&mut DefaultHasher::new()) == state2.hash(&mut DefaultHasher::new()))
}
