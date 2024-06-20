use std::pin::pin;

use flourish::{subject, Subject};
use flourish_extra::{debounce, raw_debounce};

#[test]
fn debounce_test() {
    let (get, set) = Subject::new(0).into_get_set();
    let debounced = debounce(move || get());
	//TODO
}
