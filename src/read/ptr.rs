use std::ops::Deref;
use std::marker::PhantomData;
use std::mem;


// similar to std::ptr::Shared that is however unstable
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

    #[allow(dead_code)]
    pub unsafe fn resolve<'a>(&self) -> &'a T {
       mem::transmute(self.pointer) 
    }

    pub unsafe fn resolve_mut<'a>(&mut self) -> &'a mut T {
       mem::transmute(self.pointer) 
    }
}

impl<T: ?Sized> Deref for Shared<T> {
    type Target = *mut T;

    fn deref(&self) -> &*mut T {
        unsafe { mem::transmute(&self.pointer) }
    }
}

impl<T: ?Sized> Copy for Shared<T> {}

impl<T: ?Sized> Clone for Shared<T> {
    fn clone(&self) -> Self {
        *self
    }
}
