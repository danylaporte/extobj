use crate::obj::ObjInner;
use fxhash::FxHashMap;
use std::{any::TypeId, collections::hash_map::Entry};

#[derive(Default)]
pub struct TyStore(FxHashMap<TypeId, ObjInner>);

impl TyStore {
    pub fn new() -> Self {
        Self(Default::default())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn get<T>(&self) -> Option<&T>
    where
        T: Sized + 'static,
    {
        let id = TypeId::of::<T>();

        self.0.get(&id).map(|o| unsafe { o.get_unchecked() })
    }

    pub fn get_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Sized + 'static,
    {
        let id = TypeId::of::<T>();

        match self.0.get_mut(&id) {
            Some(o) => Some(unsafe { o.get_unchecked_mut() }),
            None => None,
        }
    }

    pub fn get_mut_or_init<T, F>(&mut self, init: F) -> &mut T
    where
        F: FnOnce() -> T,
        T: Sized + 'static,
    {
        let id = TypeId::of::<T>();
        let o = self.0.entry(id).or_insert_with(|| ObjInner::new(init()));

        unsafe { o.get_unchecked_mut() }
    }

    pub fn insert<T>(&mut self, val: T) -> (&mut T, bool)
    where
        T: Sized + 'static,
    {
        let id = TypeId::of::<T>();

        let (o, inserted) = match self.0.entry(id) {
            Entry::Occupied(o) => (o.into_mut(), false),
            Entry::Vacant(v) => (v.insert(ObjInner::new(val)), true),
        };

        (unsafe { o.get_unchecked_mut() }, inserted)
    }

    pub fn take<T>(&mut self) -> Option<T>
    where
        T: Sized + 'static,
    {
        let id = TypeId::of::<T>();

        self.0
            .remove(&id)
            .map(|v| unsafe { v.into_inner_unchecked() })
    }
}

#[cfg(test)]
mod tests {
    use super::TyStore;

    #[test]
    fn it_works() {
        let mut store = TyStore::new();

        assert!(store.get::<&str>().is_none());
        assert!(store.get_mut::<&str>().is_none());

        assert_eq!(store.insert::<&str>("hello"), (&mut "hello", true));
        assert_eq!(store.insert::<&str>("world"), (&mut "hello", false));

        assert_eq!(store.get::<&str>(), Some(&"hello"));
        assert_eq!(store.get_mut::<&str>(), Some(&mut "hello"));

        assert_eq!(store.take::<&str>(), Some("hello"));
        assert_eq!(store.take::<&str>(), None);
    }
}
