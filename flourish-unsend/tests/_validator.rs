#![allow(dead_code)]

use std::{cell::RefCell, collections::VecDeque, fmt::Debug};

pub struct Validator<T>(RefCell<VecDeque<T>>);

impl<T> Validator<T> {
	#[allow(clippy::new_without_default)]
	pub const fn new() -> Self {
		Self(RefCell::new(VecDeque::new()))
	}

	pub fn push(&self, value: T) {
		self.0.try_borrow_mut().unwrap().push_back(value);
	}

	#[track_caller]
	pub fn expect(&self, expected: impl IntoIterator<Item = T>)
	where
		T: Debug + Eq,
	{
		let mut binding = self.0.try_borrow_mut().unwrap();
		let mut a = binding.drain(..);
		let mut b = expected.into_iter();
		loop {
			match (a.next(), b.next()) {
				(None, None) => break,
				(a, b) => assert_eq!(a, b),
			}
		}
	}
}
