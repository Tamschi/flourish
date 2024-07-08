#![warn(clippy::pedantic)]
#![warn(unreachable_pub)]

#[allow(async_fn_in_trait)]
pub mod prelude {
    use std::ops::{AddAssign, Sub};

    use ext_trait::extension;
    use flourish::{SignalRuntimeRef, SignalSR, SubscriptionSR};
    use flourish_extra::{
        delta,
        future::{filter, flatten_some, skip_while},
        sparse_tally,
    };
    use num_traits::Zero;

    //TODO: These have extraneous bounds that aren't really needed, usually `T: Sync + Copy`.

    #[extension(pub trait SignalExt)]
    impl<'a, T: 'a + Send + ?Sized, SR: 'a + SignalRuntimeRef> SignalSR<'a, T, SR> {
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

    #[extension(pub trait SubscriptionExt)]
    impl<'a, T: 'a + Send + Sync + ?Sized + Clone, SR: 'a + SignalRuntimeRef>
        SubscriptionSR<'a, T, SR>
    {
        async fn skip_while(
            fn_pin: impl 'a + Send + FnMut() -> T,
            predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
        ) -> SubscriptionSR<'a, T, SR>
        where
            SR: Default,
        {
            Self::skip_while_with_runtime(fn_pin, predicate_fn_pin, SR::default()).await
        }

        async fn skip_while_with_runtime(
            fn_pin: impl 'a + Send + FnMut() -> T,
            predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
            runtime: SR,
        ) -> SubscriptionSR<'a, T, SR> {
            skip_while(fn_pin, predicate_fn_pin, runtime).await
        }

        async fn filter(
            fn_pin: impl 'a + Send + FnMut() -> T,
            predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
        ) -> SubscriptionSR<'a, T, SR>
        where
            T: Copy,
            SR: Default,
        {
            Self::filter_with_runtime(fn_pin, predicate_fn_pin, SR::default()).await
        }

        async fn filter_with_runtime(
            fn_pin: impl 'a + Send + FnMut() -> T,
            predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
            runtime: SR,
        ) -> SubscriptionSR<'a, T, SR>
        where
            T: Copy,
        {
            filter(fn_pin, predicate_fn_pin, runtime).await
        }

        async fn flatten_some(
            fn_pin: impl 'a + Send + FnMut() -> Option<T>,
        ) -> SubscriptionSR<'a, T, SR>
        where
            T: Copy,
            SR: Default,
        {
            Self::flatten_some_with_runtime(fn_pin, SR::default()).await
        }

        async fn flatten_some_with_runtime(
            fn_pin: impl 'a + Send + FnMut() -> Option<T>,
            runtime: SR,
        ) -> SubscriptionSR<'a, T, SR>
        where
            T: Copy,
        {
            flatten_some(fn_pin, runtime).await
        }
    }
}
