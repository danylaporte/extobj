use std::{any::TypeId, marker::PhantomData};

/// A type-erased, owned value.
///
/// `DynObj` behaves like a `Box<dyn Any>` but is implemented entirely with
/// raw pointers so that no v-table is stored.  
/// The concrete type is known only at construction time (via `new`) and must
/// be re-supplied by the caller when the value is accessed (via `get`,
/// `get_mut`, or `into_inner`).
///
/// # Safety
///
/// * The object must **always** be dropped with the same type `T` that was
///   used to create it.  
/// * Accessing the value with the wrong type (`get::<U>` when the object was
///   created with `new::<T>`) is **instant undefined behaviour**.  
/// * In `debug_assertions` builds the actual `TypeId` is stored and an
///   assertion will catch mismatched types, but this check is **not present**
///   in release builds.
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
#[repr(C)] // not strictly required, but keeps layout predictable
pub struct DynObj {
    /// Pointer to the heap-allocated value.
    ///
    /// * Erased to `*mut ()` so the concrete type is not visible in this
    ///   struct definition.
    /// * Must be cast back to the original type before dereferencing.
    data: *mut (),

    /// Type-erased destructor that reconstructs the original `Box<T>` and
    /// drops it.
    drop: unsafe fn(*mut ()),

    /// Only present when `debug_assertions` is enabled.
    /// Stores the exact `TypeId` of the value inside `data` so that misuse
    /// can be detected at runtime in debug builds.
    #[cfg(debug_assertions)]
    tid: TypeId,

    /// Marker telling the compiler (and the drop checker) that this struct
    /// logically owns a heap-allocated value of *some* type.
    ///
    /// The actual type is erased, but `PhantomData<*mut ()>` conveys:
    /// * The value is **owned** (not merely borrowed).
    /// * The value may be aliased and moved.
    _marker: PhantomData<*mut ()>,
}

impl DynObj {
    /// Constructs a new `DynObj` that owns `val`.
    ///
    /// The value is moved onto the heap.  
    /// The type `T` is *erased* at compile time; you must remember it when
    /// you later call `get`, `get_mut`, or `into_inner`.
    ///
    /// # Examples
    ///
    /// ```
    /// let boxed = extobj::DynObj::new(vec![1, 2, 3]);
    /// ```
    pub fn new<T: 'static>(val: T) -> Self {
        let b = Box::into_raw(Box::new(val)) as *mut ();

        /// Type-erased drop glue.
        ///
        /// # Safety
        /// `p` must be the same pointer returned earlier by
        /// `Box::into_raw::<T>`.
        unsafe fn dropper<T>(p: *mut ()) {
            unsafe {
                drop(Box::from_raw(p as *mut T));
            }
        }

        Self {
            data: b,
            drop: dropper::<T>,

            #[cfg(debug_assertions)]
            tid: TypeId::of::<T>(),

            _marker: PhantomData,
        }
    }

    /// Immutably borrows the contained value, asserting that it is of type `T`.
    ///
    /// # Safety
    /// Calling this function with the wrong type is **undefined behaviour**.
    /// In builds with `debug_assertions` an assertion will catch the mismatch;
    /// in release builds no check is performed.
    ///
    /// # Examples
    ///
    /// ```
    /// let obj = extobj::DynObj::new(42u32);
    /// let n: &u32 = unsafe { obj.get() };
    /// assert_eq!(*n, 42);
    /// ```
    pub unsafe fn get<T: 'static>(&self) -> &T {
        #[cfg(debug_assertions)]
        debug_assert_eq!(self.tid, TypeId::of::<T>());
        unsafe { &*(self.data as *const T) }
    }

    /// Mutably borrows the contained value, asserting that it is of type `T`.
    ///
    /// # Safety
    /// Same caveats as [`get`](Self::get).
    ///
    /// # Examples
    ///
    /// ```
    /// let mut obj = extobj::DynObj::new(String::from("hi"));
    /// let s: &mut String = unsafe { obj.get_mut() };
    /// s.push_str("!");
    /// ```
    pub unsafe fn get_mut<T: 'static>(&mut self) -> &mut T {
        #[cfg(debug_assertions)]
        debug_assert_eq!(self.tid, TypeId::of::<T>());
        unsafe { &mut *(self.data as *mut T) }
    }

    /// Consumes this `DynObj` and returns the owned value.
    ///
    /// The value is moved out of the heap allocation and back onto the stack.
    ///
    /// # Safety
    /// Same caveats as [`get`](Self::get).
    ///
    /// # Examples
    ///
    /// ```
    /// let obj = extobj::DynObj::new(vec![1, 2]);
    /// let v: Vec<i32> = unsafe { obj.into_inner() };
    /// assert_eq!(v, [1, 2]);
    /// ```
    pub unsafe fn into_inner<T: 'static>(self) -> T {
        #[cfg(debug_assertions)]
        debug_assert_eq!(self.tid, TypeId::of::<T>());

        let out = unsafe { *Box::from_raw(self.data as *mut T) };
        std::mem::forget(self); // prevent the destructor from running
        out
    }
}

impl Drop for DynObj {
    /// Runs the type-erased destructor stored in `self.drop`.
    ///
    /// # Safety
    /// The pointer `self.data` is guaranteed to be valid for the original
    /// type because `new` paired it with an appropriate `drop` function.
    fn drop(&mut self) {
        unsafe { (self.drop)(self.data) }
    }
}
