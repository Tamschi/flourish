pub mod prelude {
    use std::ops::Sub;

    use ext_trait::extension;
    use flourish::{raw::computed_uncached_mut, SignalRuntimeRef, SignalSR};
    use flourish_extra::{debounce, delta};
    use num_traits::Zero;

	//TODO: These have extraneous bounds that aren't really needed, usually `T: Sync + Copy`.

    #[extension(pub trait SignalExt)]
    impl<'a, T: 'a + Send + ?Sized, SR: 'a + SignalRuntimeRef> SignalSR<'a, T, SR> {
        fn debounce(f: impl 'a + Send + FnMut() -> T) -> Self
        where
            T: Sync + Copy + PartialEq,
            SR: Default,
        {
            Self::debounce_with_runtime(f, SR::default())
        }

        fn debounce_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
        where
            T: Sync + Copy + PartialEq,
        {
            Self::new(debounce(computed_uncached_mut(f, runtime)))
        }

        fn delta(f: impl 'a + Send + FnMut() -> T) -> SignalSR<'a, T::Output, SR>
        where
            T: Sync + Copy + Sub<Output: Zero + Send + Sync + Copy>,
            SR: Default,
        {
            Self::delta_with_runtime(f, SR::default())
        }

        fn delta_with_runtime(
            f: impl 'a + Send + FnMut() -> T,
            runtime: SR,
        ) -> SignalSR<'a, T::Output, SR>
        where
            T: Sync + Copy + Sub<Output: Zero + Send + Sync + Copy>,
        {
            SignalSR::new(delta(computed_uncached_mut(f, runtime)))
        }
    }
}
