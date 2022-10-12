use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::{self, Debug, Display};
use std::ptr::eq as ptr_eq;
use std::str::FromStr;
use std::sync::RwLock;

use serde::de::{DeserializeOwned, Visitor};
use serde::Deserialize;

use crate::JsonValue;

#[derive(Clone, Copy, Eq, Hash, PartialOrd, Ord)]
pub struct IString(&'static str);

lazy_static::lazy_static! {
	static ref internedStrings: RwLock<HashSet<&'static str>> = RwLock::new(HashSet::new());
}

thread_local! {
	static lowercaseBuffer: RefCell<String> = RefCell::new(String::new());
}

impl IString {
	pub fn from_static(str: &'static str) -> Self {
		Self::get_or_insert(StrSrc::Static(str))
	}

	pub fn lowercased(str: &str) -> Self {
		lowercaseBuffer.with(|cell| {
			let mut buffer = cell.borrow_mut();
			buffer.clear();
			buffer.push_str(str);
			buffer.make_ascii_lowercase();
			buffer.as_str().into()
		})
	}

	fn get_or_insert(str: StrSrc) -> Self {
		let set = internedStrings
			.read()
			.expect("failed to lock interned strings cache for read");
		if let Some(&ptr) = set.get(str.borrow()) {
			return Self(ptr);
		}

		drop(set);
		let mut set = internedStrings
			.write()
			.expect("failed to lock interned strings cache for write");
		let new = str.intern();
		set.insert(new);
		Self(new)
	}

	pub fn as_str(&self) -> &'static str {
		self.0
	}
}

impl From<&str> for IString {
	fn from(str: &str) -> Self {
		Self::get_or_insert(StrSrc::Borrowed(str))
	}
}

impl From<String> for IString {
	fn from(str: String) -> Self {
		Self::get_or_insert(StrSrc::Owned(str))
	}
}

impl std::ops::Deref for IString {
	type Target = str;

	fn deref(&self) -> &'static Self::Target {
		self.0
	}
}

impl std::borrow::Borrow<str> for IString {
    fn borrow(&self) -> &'static str {
        self.0
    }
}

impl PartialEq for IString {
	fn eq(&self, other: &Self) -> bool {
		ptr_eq(self.0, other.0)
	}
}

impl Display for IString {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.0)
	}
}

impl Debug for IString {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}", self.0)
	}
}

struct IStrVisitor;

impl<'de> Visitor<'de> for IStrVisitor {
	type Value = IString;

	fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(formatter, "a string")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: serde::de::Error,
	{
		Ok(IString::lowercased(v))
	}
}

// derive macro infers wrong lifetimes :(
impl<'de> Deserialize<'de> for IString {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		deserializer.deserialize_str(IStrVisitor)
	}
}

#[test]
fn test_istring() {
	let literal = "istr_test_foo";
	let str1: IString = literal.into();
	assert!(!ptr_eq(literal, str1.0));
	let str2 = IString::from(String::from(literal).as_str());
	assert!(ptr_eq(str1.0, str2.0));

	let literal = "istr_test_bar";
	let str1 = IString::from_static(literal);
	assert!(ptr_eq(literal, str1.0));
	let str2 = IString::from(String::from(literal).as_str());
	assert!(ptr_eq(str1.0, str2.0));
	let str3 = IString::from(String::from(literal));
	assert!(ptr_eq(str1.0, str3.0));

	let owned = String::from("istr_test_baz");
	let ptr = owned.as_str() as *const str;
	let str1: IString = owned.into();
	assert!(ptr_eq(ptr, str1.0));
}

enum StrSrc<'a> {
	Borrowed(&'a str),
	Static(&'static str),
	Owned(String),
}

impl<'a> StrSrc<'a> {
	fn borrow<'b>(&'b self) -> &'a str
	where
		'b: 'a,
	{
		match self {
			Self::Borrowed(str) => str,
			Self::Static(str) => str,
			Self::Owned(str) => str,
		}
	}

	fn intern(self) -> &'static str {
		let str = match self {
			Self::Borrowed(str) => String::from(str),
			Self::Static(str) => return str,
			Self::Owned(str) => str,
		};
		Box::leak(str.into_boxed_str())
	}
}
