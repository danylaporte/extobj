use std::any::TypeId;

pub struct Obj {
    inner: ObjInner,
    tid: TypeId,
}

impl Obj {
    pub fn new<T>(val: T) -> Self
    where
        T: Sized + 'static,
    {
        Self {
            inner: ObjInner::new(val),
            tid: TypeId::of::<T>(),
        }
    }

    #[inline]
    pub fn try_get<T>(&self) -> Option<&T>
    where
        T: Sized + 'static,
    {
        if TypeId::of::<T>() == self.tid {
            Some(unsafe { self.inner.get_unchecked::<T>() })
        } else {
            None
        }
    }

    #[inline]
    pub fn try_get_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Sized + 'static,
    {
        if TypeId::of::<T>() == self.tid {
            Some(unsafe { self.inner.get_unchecked_mut::<T>() })
        } else {
            None
        }
    }

    pub fn try_into_inner<T>(self) -> Result<T, Self>
    where
        T: Sized + 'static,
    {
        if TypeId::of::<T>() == self.tid {
            Ok(unsafe { self.inner.into_inner_unchecked() })
        } else {
            Err(self)
        }
    }
}

pub struct UnsafeObj {
    inner: ObjInner,

    #[cfg(debug_assertions)]
    tid: TypeId,
}

impl UnsafeObj {
    pub fn new<T>(val: T) -> Self
    where
        T: Sized + 'static,
    {
        Self {
            inner: ObjInner::new(val),

            #[cfg(debug_assertions)]
            tid: TypeId::of::<T>(),
        }
    }

    /// # Safety
    ///
    /// The caller must ensure the specified type must be the same used when calling [UnsafeObj::new].
    #[inline]
    pub unsafe fn get_unchecked<T>(&self) -> &T
    where
        T: Sized + 'static,
    {
        #[cfg(debug_assertions)]
        debug_assert_eq!(TypeId::of::<T>(), self.tid);

        self.inner.get_unchecked()
    }

    /// # Safety
    ///
    /// The caller must ensure the specified type must be the same used when calling [UnsafeObj::new].
    #[inline]
    pub unsafe fn get_unchecked_mut<T>(&mut self) -> &mut T
    where
        T: Sized + 'static,
    {
        #[cfg(debug_assertions)]
        debug_assert_eq!(TypeId::of::<T>(), self.tid);

        self.inner.get_unchecked_mut()
    }

    /// # Safety
    ///
    /// The caller must ensure the specified type must be the same used when calling [UnsafeObj::new].
    pub unsafe fn into_inner_unchecked<T>(self) -> T
    where
        T: Sized + 'static,
    {
        #[cfg(debug_assertions)]
        debug_assert_eq!(TypeId::of::<T>(), self.tid);

        self.inner.into_inner_unchecked()
    }
}

pub(crate) struct ObjInner {
    dropper: Option<Box<dyn FnOnce(usize)>>,
    ptr: usize,
}

impl ObjInner {
    pub(crate) fn new<T>(val: T) -> Self
    where
        T: Sized + 'static,
    {
        Self {
            dropper: Some(Box::new(|ptr| unsafe {
                // drop the value
                let _ = Box::from_raw(ptr as *mut T);
            })),
            ptr: Box::into_raw(Box::new(val)) as usize,
        }
    }

    #[inline]
    pub(crate) unsafe fn get_unchecked<T>(&self) -> &T
    where
        T: Sized + 'static,
    {
        &*(self.ptr as *const T)
    }

    #[inline]
    pub(crate) unsafe fn get_unchecked_mut<T>(&mut self) -> &mut T
    where
        T: Sized + 'static,
    {
        &mut *(self.ptr as *mut T)
    }

    pub(crate) unsafe fn into_inner_unchecked<T>(mut self) -> T
    where
        T: Sized + 'static,
    {
        self.dropper = None;
        *Box::from_raw(self.ptr as *mut T)
    }
}

impl Drop for ObjInner {
    fn drop(&mut self) {
        if let Some(dropper) = self.dropper.take() {
            dropper(self.ptr);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::obj::ObjInner;

    #[test]
    fn obj_get() {
        let mut obj = ObjInner::new::<&str>("hello");

        assert_eq!(*unsafe { obj.get_unchecked::<&str>() }, "hello");
        assert_eq!(*unsafe { obj.get_unchecked_mut::<&str>() }, "hello");
        assert_eq!(unsafe { obj.into_inner_unchecked::<&str>() }, "hello");
    }
}
