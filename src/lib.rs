#![allow(dead_code, non_snake_case, non_upper_case_globals)]

use std::sync::{Arc, RwLock};

pub mod loader;
pub mod types;
pub mod world;

// in future this may need to become its own type, but for now
// writing the derefs et. al is too much work
pub type Shared<T> = Arc<RwLock<T>>;

pub fn make_shared<T>(v: T) -> Shared<T> {
	Arc::new(RwLock::new(v))
}

pub fn clone_shared<T>(v: &Shared<T>) -> Shared<T> {
	Arc::clone(v)
}
