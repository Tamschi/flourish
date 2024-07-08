use flourish::{shadow_clone, Announcer, Effect, SourcePin, Subscription};
mod _validator;
use _validator::Validator;

#[test]
fn set() {
	let v = &Validator::new();

	let a = Announcer::new("a");
	let b = Announcer::new("b");
	let _sub_a = Subscription::computed({
		shadow_clone!(a);
		move || v.push(("_sub_a", a.get()))
	});
	let _sub_b = Subscription::computed({
		shadow_clone!(b);
		move || v.push(("_sub_b", b.get()))
	});
	let _effect = Effect::new(
		{
			shadow_clone!(a, b);
			move || b.replace(a.get())
		},
		drop,
	);
	v.expect([("_sub_a", "a"), ("_sub_b", "b"), ("_sub_b", "a")]);

	a.replace_blocking("aa");

	v.expect([("_sub_a", "aa"), ("_sub_b", "aa")]);
}
