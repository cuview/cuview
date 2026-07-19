#![allow(unused)]

pub(crate) type JsonValue = serde_json::value::Value;

pub mod jarfs;
pub mod loader;
pub mod renderer;
pub mod types;
pub mod world;

pub fn default<T: Default>() -> T {
	T::default()
}
