pub mod prelude {
    use std::ops::{AddAssign, Sub};

    use ext_trait::extension;
    use flourish::{SignalRuntimeRef, SignalSR};
    use flourish_extra::{debounce, delta, sparse_tally};
    use num_traits::Zero;

    //TODO: These have extraneous bounds that aren't really needed, usually `T: Sync + Copy`.

    #[extension(pub trait SignalExt)]
    impl<'a, T: 'a + Send + ?Sized, SR: 'a + SignalRuntimeRef> SignalSR<'a, T, SR> {
        fn debounce(fn_pin: impl 'a + Send + FnMut() -> T) -> Self
        where
            T: Sync + Copy + PartialEq,
            SR: Default,
        {
            Self::debounce_with_runtime(fn_pin, SR::default())
        }

        fn debounce_with_runtime(fn_pin: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
        where
            T: Sync + Copy + PartialEq,
        {
            Self::new(debounce(fn_pin, runtime))
        }

        fn delta<V: 'a + Send>(fn_pin: impl 'a + Send + FnMut() -> V) -> SignalSR<'a, T, SR>
        where
            T: Zero,
            for<'b> &'b V: Sub<Output = T>,
            SR: Default,
        {
            Self::delta_with_runtime(fn_pin, SR::default())
        }

        fn delta_with_runtime<V: 'a + Send>(
            fn_pin: impl 'a + Send + FnMut() -> V,
            runtime: SR,
        ) -> SignalSR<'a, T, SR>
        where
            T: Zero,
            for<'b> &'b V: Sub<Output = T>,
        {
            SignalSR::new(delta(fn_pin, runtime))
        }

        fn sparse_tally<V: 'a + Send>(fn_pin: impl 'a + Send + FnMut() -> V) -> SignalSR<'a, T, SR>
        where
            T: Zero + Send + AddAssign<V>,
            SR: Default,
        {
            Self::sparse_tally_with_runtime(fn_pin, SR::default())
        }

        fn sparse_tally_with_runtime<V: 'a + Send>(
            fn_pin: impl 'a + Send + FnMut() -> V,
            runtime: SR,
        ) -> SignalSR<'a, T, SR>
        where
            T: Zero + Send + AddAssign<V>,
        {
            SignalSR::new(sparse_tally(fn_pin, runtime))
        }
    }
}
