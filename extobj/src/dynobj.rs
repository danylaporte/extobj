use std::{
    marker::PhantomData,
    mem::{self, MaybeUninit, align_of, needs_drop, size_of},
    ptr,
};

/// Raw storage: either an erased `Box<T>` pointer or, for small `T`, the
/// value itself stored inline.
type Slot = MaybeUninit<*mut ()>;

/// `T` is stored inline when it fits in a pointer-sized, pointer-aligned slot.
/// This is a compile-time constant for every `T`, so the branch on it in
/// `new`/`get`/`into_inner` folds away.
#[inline(always)]
const fn is_inline<T>() -> bool {
    size_of::<T>() <= size_of::<Slot>() && align_of::<T>() <= align_of::<Slot>()
}

/// A type-erased, owned value.
///
/// `DynObj` behaves like a `Box<dyn Any>` but stores no v-table.  The
/// concrete type is known only at construction (`new`) and must be
/// re-supplied by the caller on access (`get`, `get_mut`, `into_inner`).
///
/// Values that fit in a pointer (`u8`..`u64`, `f64`, `bool`, thin pointers,
/// small enums, ...) are stored **inline** and never touch the heap; larger
/// values are boxed.  Types without drop glue skip the destructor call.
///
/// # Safety
///
/// * Accessing the value with a type other than the one passed to `new`
///   (e.g. `get::<U>` after `new::<T>`) is **undefined behavior**.
/// * In `debug_assertions` builds the `TypeId` is recorded and mismatches
///   panic; release builds perform no check.
///
/// # Example
///
/// ```
/// let mut obj = extobj::DynObj::new(String::from("hello"));
/// let s: &mut String = unsafe { obj.get_mut() };
/// s.push_str(" world");
/// let s: String = unsafe { obj.into_inner() };
/// assert_eq!(s, "hello world");
/// ```
#[repr(C)]
pub struct DynObj {
    data: Slot,

    /// Destructor for `data`; `None` when nothing needs to run on drop.
    drop: Option<unsafe fn(*mut Slot)>,

    #[cfg(debug_assertions)]
    tid: std::any::TypeId,

    /// Owns an erased value: not `Send`/`Sync` by default (opted in below),
    /// and `!Unpin`-agnostic like a raw pointer.
    _marker: PhantomData<*mut ()>,
}

unsafe fn drop_inline<T>(slot: *mut Slot) {
    unsafe { ptr::drop_in_place(slot.cast::<T>()) }
}

unsafe fn drop_boxed<T>(slot: *mut Slot) {
    unsafe { drop(Box::from_raw((*slot).assume_init().cast::<T>())) }
}

impl DynObj {
    /// Constructs a new `DynObj` that owns `val`.
    ///
    /// Small values are stored inline; larger ones are moved to the heap.
    ///
    /// # Examples
    ///
    /// ```
    /// let boxed = extobj::DynObj::new(vec![1, 2, 3]);
    /// let inline = extobj::DynObj::new(7u32);
    /// ```
    #[inline]
    pub fn new<T>(val: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        let mut data = Slot::uninit();

        let drop: Option<unsafe fn(*mut Slot)> = if is_inline::<T>() {
            unsafe { data.as_mut_ptr().cast::<T>().write(val) };
            needs_drop::<T>().then_some(drop_inline::<T> as unsafe fn(*mut Slot))
        } else {
            data.write(Box::into_raw(Box::new(val)).cast());
            Some(drop_boxed::<T>)
        };

        Self {
            data,
            drop,
            #[cfg(debug_assertions)]
            tid: std::any::TypeId::of::<T>(),
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    fn check<T: 'static>(&self, _what: &str) {
        #[cfg(debug_assertions)]
        assert_eq!(
            self.tid,
            std::any::TypeId::of::<T>(),
            "Type mismatch in DynObj::{_what}"
        );
    }

    /// Pointer to the stored `T`, wherever it lives.
    #[inline(always)]
    fn ptr<T>(&self) -> *mut T {
        if is_inline::<T>() {
            self.data.as_ptr().cast_mut().cast()
        } else {
            unsafe { self.data.assume_init() }.cast()
        }
    }

    /// Immutably borrows the contained value as `&T`.
    ///
    /// # Safety
    /// `T` must be the type passed to `new`.
    ///
    /// # Examples
    ///
    /// ```
    /// let obj = extobj::DynObj::new(42u32);
    /// let n: &u32 = unsafe { obj.get() };
    /// assert_eq!(*n, 42);
    /// ```
    #[inline]
    pub unsafe fn get<T>(&self) -> &T
    where
        T: Send + 'static,
    {
        self.check::<T>("get");
        unsafe { &*self.ptr::<T>() }
    }

    /// Mutably borrows the contained value as `&mut T`.
    ///
    /// # Safety
    /// `T` must be the type passed to `new`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut obj = extobj::DynObj::new(String::from("hi"));
    /// let s: &mut String = unsafe { obj.get_mut() };
    /// s.push_str("!");
    /// ```
    #[inline]
    pub unsafe fn get_mut<T>(&mut self) -> &mut T
    where
        T: Send + 'static,
    {
        self.check::<T>("get_mut");
        unsafe { &mut *self.ptr::<T>() }
    }

    /// Consumes `DynObj` and returns the owned `T`.
    ///
    /// # Safety
    /// `T` must be the type passed to `new`.
    ///
    /// # Examples
    ///
    /// ```
    /// let obj = extobj::DynObj::new(vec![1, 2]);
    /// let v: Vec<i32> = unsafe { obj.into_inner() };
    /// assert_eq!(v, [1, 2]);
    /// ```
    #[inline]
    pub unsafe fn into_inner<T>(self) -> T
    where
        T: Send + 'static,
    {
        self.check::<T>("into_inner");

        let out = if is_inline::<T>() {
            unsafe { self.ptr::<T>().read() }
        } else {
            unsafe { *Box::from_raw(self.ptr::<T>()) }
        };

        // Ownership has been moved out; the destructor must not run.
        mem::forget(self);
        out
    }
}

impl Drop for DynObj {
    #[inline]
    fn drop(&mut self) {
        if let Some(drop) = self.drop {
            unsafe { drop(&mut self.data) }
        }
    }
}

// SAFETY: `new` requires `T: Send + Sync`, so the erased value is too.
unsafe impl Send for DynObj {}
unsafe impl Sync for DynObj {}
