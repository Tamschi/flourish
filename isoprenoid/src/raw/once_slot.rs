use std::{cell::OnceCell, sync::Mutex};

#[derive(Debug)]
pub(super) struct OnceSlot<T> {
	critical: Mutex<()>,
	value: OnceCell<T>,
}

impl<T> OnceSlot<T> {
	#[must_use]
	pub(super) const fn new() -> Self {
		Self {
			critical: Mutex::new(()),
			value: OnceCell::new(),
		}
	}

	/// This method becomes reentrant once the [`OnceCell`] is initialised.
	pub(super) fn get_or_write(&self, f: impl FnOnce(&OnceCell<T>)) -> &T {
		if let Some(value) = self.value.get() {
			value
		} else {
			let _guard = self.critical.lock().unwrap();
			if let Some(value) = self.value.get() {
				value
			} else {
				f(&self.value);
				if let Some(value) = self.value.get() {
					value
				} else {
					panic!("`f` didn't write the value.")
				}
			}
		}
	}

	pub(super) fn get(&self) -> Option<&T> {
		self.value.get()
	}

	pub(super) fn get_mut(&mut self) -> Option<&mut T> {
		self.value.get_mut()
	}
}
