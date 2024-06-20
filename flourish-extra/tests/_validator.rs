use std::{collections::VecDeque, fmt::Debug, sync::Mutex};

pub struct Validator<T>(Mutex<VecDeque<T>>);

impl<T> Validator<T> {
    pub const fn new() -> Self {
        Self(Mutex::new(VecDeque::new()))
    }

    pub fn push(&self, value: T) {
        self.0.lock().unwrap().push_back(value);
    }

    #[track_caller]
    pub fn expect(&self, expected: impl IntoIterator<Item = T>)
    where
        T: Debug + Eq,
    {
        let mut binding = self.0.lock().unwrap();
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
