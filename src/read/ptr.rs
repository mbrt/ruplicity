use std::ops::Deref;
use std::marker::PhantomData;
use std::mem;


// similar to std::ptr::Shared that is however unstable
#[derive(Copy, Clone)]
pub struct Shared<T: ?Sized> {
    pointer: *const T,
    _marker: PhantomData<T>,
}


impl<T: ?Sized> Shared<T> {
    pub unsafe fn new(ptr: *const T) -> Self {
        Shared {
            pointer: ptr,
            _marker: PhantomData,
        }
    }
}

impl<T: ?Sized> Deref for Shared<T> {
    type Target = *mut T;

    fn deref(&self) -> &*mut T {
        unsafe { mem::transmute(&self.pointer) }
    }
}
