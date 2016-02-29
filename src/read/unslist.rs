use std::ops::Deref;
use std::marker::PhantomData;
use std::mem;


pub struct UnsafeList<T> {
    length: usize,
    head: Link<T>,
    tail: RawLink<T>,
}

pub struct Node<T> {
    next: Link<T>,
    prev: RawLink<T>,
    value: T,
}


type Link<T> = Option<Box<Node<T>>>;
type RawLink<T> = Option<Shared<Node<T>>>;


// similar to std::ptr::Shared that is however unstable
#[derive(Copy, Clone)]
struct Shared<T: ?Sized> {
    pointer: *const T,
    _marker: PhantomData<T>,
}


impl<T> UnsafeList<T> {
    pub fn new() -> Self {
        UnsafeList {
            length: 0,
            head: None,
            tail: None,
        }
    }

    pub fn push_front(&mut self, val: T) {
        let mut new_head = Box::new(Node::new(val));
        match self.head {
            None => {
            }
            Some(ref mut head) => {
                new_head.prev = None;
                //head.prev = raw_link_from_link(&mut self.head);
                mem::swap(head, &mut new_head);
                head.next = Some(new_head);
            }
        }
    }
}


impl<T> Node<T> {
    fn new(val: T) -> Self {
        Node {
            next: None,
            prev: None,
            value: val,
        }
    }
}



impl<T: ?Sized> Shared<T> {
    pub unsafe fn new(ptr: *mut T) -> Self {
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


fn raw_link_from_link<T>(link: &mut Link<T>) -> RawLink<T> {
    unimplemented!()
    //link.as_mut().map(|bnode| unsafe { Shared::new(&mut bnode) })
}
