use std::borrow::Borrow;

pub trait Source {
    type Value: ?Sized;

    fn get(&self) -> Self::Value
    where
        Self::Value: Sync + Copy;

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone;

    fn get_exclusive(&self) -> Self::Value
    where
        Self::Value: Copy;

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Copy;

    fn read(&self) -> Box<dyn '_ + Borrow<Self::Value>>
    where
        Self::Value: Sync;
}

pub trait DelegateSource {
    type DelegateValue: ?Sized;

    fn delegate_source(&self) -> &impl Source<Value = Self::DelegateValue>;
}

impl<T: ?Sized + DelegateSource> Source for T {
    type Value = <T as DelegateSource>::DelegateValue;

    fn get(&self) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        self.delegate_source().get()
    }

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self.delegate_source().get_clone()
    }

    fn get_exclusive(&self) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.delegate_source().get_exclusive()
    }

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.delegate_source().get_clone_exclusive()
    }

    fn read(&self) -> Box<dyn '_ + Borrow<Self::Value>>
    where
        Self::Value: Sync,
    {
        self.delegate_source().read()
    }
}
