#![allow(non_snake_case, non_upper_case_globals, unused)]

pub(crate) type JsonValue = serde_json::value::Value;

pub mod jarfs;
pub mod loader;
pub mod renderer;
pub mod types;
pub mod world;
