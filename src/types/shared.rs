use std::fmt;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

pub struct Shared<T>(Arc<RwLock<T>>);

impl<T> Shared<T> {
	pub fn new(v: T) -> Self {
		Shared(Arc::new(RwLock::new(v)))
	}

	pub fn new_cyclic<F: FnOnce(WeakShared<T>) -> T>(ctor: F) -> Self {
		Shared(Arc::new_cyclic(|weak| {
			let weak = WeakShared(weak.clone());
			RwLock::new(ctor(weak))
		}))
	}

	pub fn inner(&self) -> &Arc<RwLock<T>> {
		&self.0
	}

	pub fn borrow(&self) -> RwLockReadGuard<T> {
		self.0
			.read()
			.expect("Failed to lock Shared for immutable borrow")
	}

	pub fn borrow_mut(&self) -> RwLockWriteGuard<T> {
		self.0
			.write()
			.expect("Failed to lock Shared for mutable borrow")
	}
}

impl<T> Clone for Shared<T> {
	fn clone(&self) -> Self {
		Shared(Arc::clone(&self.0))
	}
}

impl<T> From<T> for Shared<T> {
	fn from(v: T) -> Self {
		Self::new(v)
	}
}

impl<T: Debug> Debug for Shared<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Shared").field(&self.0).finish()
	}
}

pub struct WeakShared<T>(Weak<RwLock<T>>);

impl<T> WeakShared<T> {
	pub fn inner(&self) -> &Weak<RwLock<T>> {
		&self.0
	}

	pub fn upgrade(&self) -> Option<Shared<T>> {
		self.0.upgrade().map(Shared)
	}
}

impl<T> Clone for WeakShared<T> {
	fn clone(&self) -> Self {
		WeakShared(Weak::clone(&self.0))
	}
}

impl<T: Debug> Debug for WeakShared<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("WeakShared").field(&self.0).finish()
	}
}

#[test]
fn test_shared() {
	let x: Shared<i32> = 0.into();
	assert!(*x.borrow() == 0);

	let y = x.clone();
	*y.borrow_mut() = 42;
	assert!(*x.borrow() == 42);
}
