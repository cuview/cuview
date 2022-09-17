pub mod blockstate;
pub mod coords;
pub mod interned_string;
pub mod shared;

use std::cell::RefCell;

pub use coords::{BlockPos, ChunkPos, RegionPos};
pub use interned_string::IString;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
