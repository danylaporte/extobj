use std::marker::PhantomData;

/// A type-erased, owned value.
///
/// `DynObj` behaves like a `Box<dyn Any>` but is implemented entirely with
/// raw pointers to avoid storing a v-table, making it more lightweight.
/// The concrete type is known only at construction time (via `new`) and must
/// be re-supplied by the caller when accessing the value (via `get`,
/// `get_mut`, or `into_inner`).
///
/// # Safety
///
/// * The object must **always** be dropped with the same type `T` that was
///   used to create it to prevent undefined behavior.
/// * Accessing the value with the wrong type (e.g., `get::<U>` when the object was
///   created with `new::<T>`) results in **instant undefined behavior**.
/// * In `debug_assertions` builds, the actual `TypeId` is stored, and an
///   assertion will catch mismatched types, but this safety check is **not present**
///   in release builds to optimize performance.
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
#[repr(C)] // Ensures predictable memory layout for compatibility with raw pointers
pub struct DynObj {
    /// Pointer to the heap-allocated value.
    ///
    /// * Type is erased to `*mut ()` to hide the concrete type `T` in the struct.
    /// * Must be cast back to the original type (`*mut T`) before dereferencing.
    /// * Points to memory allocated by `Box::into_raw`.
    data: *mut (),

    /// Type-erased destructor function pointer.
    ///
    /// * Stores a function that reconstructs the original `Box<T>` and drops it.
    /// * Ensures proper cleanup of the heap-allocated value when `DynObj` is dropped.
    drop: unsafe fn(*mut ()),

    /// Stores the `TypeId` of the value in `data` for type safety checks.
    ///
    /// * Only included when `debug_assertions` is enabled (debug builds).
    /// * Used to verify that the type `T` provided in `get`, `get_mut`, or `into_inner`
    ///   matches the type used in `new`.
    #[cfg(debug_assertions)]
    tid: std::any::TypeId,

    /// Marker to indicate ownership of a heap-allocated value.
    ///
    /// * `PhantomData<*mut ()>` informs the compiler that `DynObj` logically owns
    ///   a heap-allocated value of some type, even though the type is erased.
    /// * Ensures proper drop checking and conveys that the value is owned, not borrowed.
    /// * Allows aliasing and moving, as the pointer is managed manually.
    _marker: PhantomData<*mut ()>,
}

impl DynObj {
    /// Constructs a new `DynObj` that owns `val`.
    ///
    /// * Moves the provided value `val` onto the heap using `Box`.
    /// * Erases the type `T` at compile time, storing only a raw pointer and a destructor.
    /// * The caller must remember the type `T` for later access via `get`, `get_mut`, or `into_inner`.
    ///
    /// # Constraints
    /// * `T` must implement `Send` to ensure thread safety for the raw pointer.
    /// * `T` must be `'static` to ensure no references to temporary data are stored.
    ///
    /// # Examples
    ///
    /// ```
    /// let boxed = extobj::DynObj::new(vec![1, 2, 3]);
    /// ```
    pub fn new<T>(val: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        // Allocate the value on the heap and convert to a raw pointer, erasing the type
        let b = Box::into_raw(Box::new(val)) as *mut ();

        /// Type-erased drop function for `T`.
        ///
        /// * Takes a raw pointer, casts it back to `*mut T`, and drops it as a `Box<T>`.
        /// * Ensures proper cleanup of the heap-allocated value.
        ///
        /// # Safety
        /// * `p` must be the same pointer returned by `Box::into_raw::<T>` during construction.
        unsafe fn dropper<T>(p: *mut ()) {
            unsafe {
                // Reconstruct the `Box<T>` from the raw pointer and drop it
                drop(Box::from_raw(p as *mut T));
            }
        }

        Self {
            data: b,            // Store the raw pointer to the heap-allocated value
            drop: dropper::<T>, // Store the type-specific drop function
            #[cfg(debug_assertions)]
            tid: std::any::TypeId::of::<T>(), // Store the TypeId for debug type checking
            _marker: PhantomData, // Initialize the ownership marker
        }
    }

    /// Immutably borrows the contained value as a reference of type `T`.
    ///
    /// * Returns a reference to the heap-allocated value, cast to `&T`.
    ///
    /// # Safety
    /// * The caller must ensure `T` matches the type used in `new`.
    /// * Using the wrong type causes **undefined behavior**.
    /// * In debug builds, a `TypeId` check ensures type safety; no check occurs in release builds.
    ///
    /// # Examples
    ///
    /// ```
    /// let obj = extobj::DynObj::new(42u32);
    /// let n: &u32 = unsafe { obj.get() };
    /// assert_eq!(*n, 42);
    /// ```
    pub unsafe fn get<T>(&self) -> &T
    where
        T: Send + 'static,
    {
        // Check type safety in debug builds
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            self.tid,
            std::any::TypeId::of::<T>(),
            "Type mismatch in DynObj::get"
        );

        // Cast the raw pointer to a reference of type `T` and return it
        unsafe { &*(self.data as *const T) }
    }

    /// Mutably borrows the contained value as a mutable reference of type `T`.
    ///
    /// * Returns a mutable reference to the heap-allocated value, cast to `&mut T`.
    ///
    /// # Safety
    /// * Same requirements as `get`: the type `T` must match the type used in `new`.
    /// * Using the wrong type causes **undefined behavior**.
    /// * In debug builds, a `TypeId` check ensures type safety; no check in release builds.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut obj = extobj::DynObj::new(String::from("hi"));
    /// let s: &mut String = unsafe { obj.get_mut() };
    /// s.push_str("!");
    /// ```
    pub unsafe fn get_mut<T>(&mut self) -> &mut T
    where
        T: Send + 'static,
    {
        // Check type safety in debug builds
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            self.tid,
            std::any::TypeId::of::<T>(),
            "Type mismatch in DynObj::get_mut"
        );

        // Cast the raw pointer to a mutable reference of type `T` and return it
        unsafe { &mut *(self.data as *mut T) }
    }

    /// Consumes `DynObj` and returns the owned value of type `T`.
    ///
    /// * Moves the heap-allocated value back to the stack as type `T`.
    /// * Prevents the `DynObj` destructor from running to avoid double-free.
    ///
    /// # Safety
    /// * Same requirements as `get`: the type `T` must match the type used in `new`.
    /// * Using the wrong type causes **undefined behavior**.
    /// * In debug builds, a `TypeId` check ensures type safety; no check in release builds.
    ///
    /// # Examples
    ///
    /// ```
    /// let obj = extobj::DynObj::new(vec![1, 2]);
    /// let v: Vec<i32> = unsafe { obj.into_inner() };
    /// assert_eq!(v, [1, 2]);
    /// ```
    pub unsafe fn into_inner<T>(self) -> T
    where
        T: Send + 'static,
    {
        // Check type safety in debug builds
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            self.tid,
            std::any::TypeId::of::<T>(),
            "Type mismatch in DynObj::into_inner"
        );

        // Reconstruct the value from the raw pointer and move it to the stack
        let out = unsafe { *Box::from_raw(self.data as *mut T) };

        // Prevent the destructor from running to avoid double-free
        std::mem::forget(self);

        out
    }
}

/// Implements the `Drop` trait to clean up the heap-allocated value.
///
/// * Calls the type-erased destructor stored in `self.drop` to free the memory.
impl Drop for DynObj {
    /// Runs the type-erased destructor stored in `self.drop`.
    ///
    /// # Safety
    /// * The pointer `self.data` is guaranteed to be valid for the original
    ///   type because `new` paired it with the correct `drop` function.
    fn drop(&mut self) {
        // Call the stored destructor function with the raw pointer
        unsafe { (self.drop)(self.data) }
    }
}

/// Marks `DynObj` as safe to send across threads.
///
/// * Safe because the value in `data` is required to implement `Send` in `new`.
/// * The raw pointer and destructor function are thread-safe as long as the
///   value itself is `Send`.
unsafe impl Send for DynObj {}

unsafe impl Sync for DynObj {}
