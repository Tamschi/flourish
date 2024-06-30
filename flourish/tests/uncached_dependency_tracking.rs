use std::sync::Mutex;

use flourish::{Signal, SourcePin as _, Subject, Subscription};
mod _validator;
use _validator::Validator;

#[test]
fn heap() {
    let v = &Validator::new();

    let (a, set_a) = Subject::new(()).into_get_set_blocking();
    let (b, set_b) = Subject::new(()).into_get_set_blocking();
    let (c, set_c) = Subject::new(()).into_get_set_blocking();

    let roundabout = Signal::computed_uncached_mut({
        let mut angle = 0;
        move || {
            match angle {
                0 => a(),
                1 => b(),
                2 => c(),
                _ => unreachable!(),
            }
            angle = (angle + 1) % 3;
        }
    });
    v.expect([]);

    let _a = Subscription::computed(|| {
        v.push('a');
        roundabout.get()
    });
    v.expect(['a']);
    let _b = Subscription::computed(|| {
        v.push('b');
        roundabout.get()
    });
    v.expect(['b']);

    // There are two subscriptions, so each "hit" advances twice.

    set_b(());
    v.expect(['a', 'b']);

    set_b(());
    set_c(());
    v.expect([]);

    set_a(());
    v.expect(['a', 'b']);
}

#[test]
fn stack() {
    let v = &Validator::new();

    let (a, set_a) = Subject::new(()).into_get_set_blocking();
    let (b, set_b) = Subject::new(()).into_get_set_blocking();
    let (c, set_c) = Subject::new(()).into_get_set_blocking();

    let roundabout = Signal::computed_uncached({
        let angle = Mutex::new(0);
        move || {
            let mut angle = angle.lock().unwrap();
            match *angle {
                0 => a(),
                1 => b(),
                2 => c(),
                _ => unreachable!(),
            }
            *angle = (*angle + 1) % 3;
        }
    });
    v.expect([]);

    let _a = Subscription::computed(|| {
        v.push('a');
        roundabout.get()
    });
    v.expect(['a']);
    let _b = Subscription::computed(|| {
        v.push('b');
        roundabout.get()
    });
    v.expect(['b']);

    // There are two subscriptions, so each "hit" advances twice.

    set_b(());
    v.expect(['a', 'b']);

    set_b(());
    set_c(());
    v.expect([]);

    set_a(());
    v.expect(['a', 'b']);
}
