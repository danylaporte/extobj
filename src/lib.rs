mod obj;
mod ty_store;

pub use obj::{Obj, UnsafeObj};
use std::{
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Index, IndexMut},
};
pub use ty_store::TyStore;

#[doc(hidden)]
pub type Defs = std::sync::RwLock<
    Vec<(
        &'static (dyn Fn() -> usize + Sync),
        &'static (dyn Fn(usize) + Sync),
    )>,
>;

/// # Note
/// This trait is for used only in macros.
#[doc(hidden)]
pub trait __ExtObjDef: 'static {
    fn defs() -> &'static Defs;
}

/// An extendable struct that be extented across crate.
pub struct ExtObj<O: __ExtObjDef>(Vec<usize>, PhantomData<O>);

impl<O: __ExtObjDef> ExtObj<O> {
    pub fn new() -> Self {
        Self(
            O::defs().read().unwrap().iter().map(|(a, _)| a()).collect(),
            PhantomData,
        )
    }

    #[inline]
    pub fn get<T>(&self, var: Var<O, T>) -> &T {
        unsafe { &*(*self.0.get_unchecked(var.0) as *const T) }
    }

    #[inline]
    pub fn get_mut<T>(&mut self, var: Var<O, T>) -> &mut T {
        unsafe { &mut *(*self.0.get_unchecked(var.0) as *mut T) }
    }
}

impl<O: __ExtObjDef> Default for ExtObj<O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: __ExtObjDef> Drop for ExtObj<O> {
    fn drop(&mut self) {
        self.0
            .iter()
            .zip(O::defs().read().unwrap().iter())
            .for_each(|(a, b)| (b.1)(*a));
    }
}

impl<O: __ExtObjDef, T> Index<Var<O, T>> for ExtObj<O> {
    type Output = T;

    #[inline]
    fn index(&self, index: Var<O, T>) -> &Self::Output {
        self.get(index)
    }
}

impl<O: __ExtObjDef, T> IndexMut<Var<O, T>> for ExtObj<O> {
    #[inline]
    fn index_mut(&mut self, index: Var<O, T>) -> &mut Self::Output {
        self.get_mut(index)
    }
}

pub struct Var<O, T>(usize, PhantomData<O>, PhantomData<T>);

impl<O, T> Clone for Var<O, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<O, T> Copy for Var<O, T> {}

impl<O, T> Eq for Var<O, T> {}

impl<O, T> Hash for Var<O, T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<O, T> PartialEq for Var<O, T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<O: __ExtObjDef, T: Default + 'static> Var<O, T> {
    #[doc(hidden)]
    pub fn __new() -> Self {
        let mut defs = O::defs().write().unwrap();
        let index = defs.len();

        defs.push((&init_default::<T>, &dropper::<T>));

        Self(index, PhantomData, PhantomData)
    }
}

fn init_default<T: Default>() -> usize {
    Box::into_raw(Box::new(T::default())) as usize
}

fn dropper<T>(ptr: usize) {
    unsafe {
        let _ = Box::from_raw(ptr as *mut T);
    }
}

#[macro_export]
macro_rules! extobj {
    ($vis:vis struct $name:ident) => {
        $vis struct $name;

        impl $crate::__ExtObjDef for $name {
            #[inline]
            fn defs() -> &'static $crate::Defs {
                static CELL: $crate::Defs = std::sync::RwLock::new(Vec::new());
                &CELL
            }
        }
    };

    (impl $ext_obj:ty {
        $($vis:vis $prop_name:ident: $prop_ty:ty,)*
    }) => {
        $(
            #[static_init::dynamic]
            static $prop_name: $crate::Var<$ext_obj, $prop_ty> = $crate::Var::__new();
        )*
    };
}

#[cfg(test)]
mod tests {
    use super::{extobj, ExtObj};

    extobj!(struct MyObj);

    extobj!(impl MyObj {
        MY_PROP: i32,
    });

    #[test]
    fn it_works() {
        let mut obj = ExtObj::<MyObj>::new();

        assert_eq!(obj[*MY_PROP], 0);

        obj[*MY_PROP] = 10;

        assert_eq!(obj[*MY_PROP], 10);
    }
}
