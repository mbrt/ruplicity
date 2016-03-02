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
struct RawLink<T>(Option<Shared<Node<T>>>);


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
            tail: RawLink::none(),
        }
    }

    pub fn push_front(&mut self, val: T) {
        let mut new_head = Box::new(Node::new(val));
        match self.head {
            None => {
                self.head = link_no_prev(new_head);
                self.tail = RawLink::from_link(&mut self.head);
            }
            Some(ref mut head) => {
                new_head.prev = RawLink::none();
                head.prev = RawLink::some(&mut *new_head);
                mem::swap(head, &mut new_head);
                head.next = Some(new_head);
            }
        }
        self.length += 1;
    }

    pub fn push_back(&mut self, val: T) {
        match unsafe { self.tail.resolve_mut() } {
            None => {
                return self.push_front(val);
            }
            Some(tail) => {
                let mut new_tail = Box::new(Node::new(val));
                tail.set_next(new_tail);
                self.tail = RawLink::from_link(&mut tail.next);
            }
        }
        self.length += 1;
    }
}


impl<T> Node<T> {
    fn new(val: T) -> Self {
        Node {
            next: None,
            prev: RawLink::none(),
            value: val,
        }
    }

    fn set_next(&mut self, mut next: Box<Node<T>>) {
        debug_assert!(self.next.is_none());
        next.prev = RawLink::some(self);
        self.next = Some(next);
    }
}


impl<T: ?Sized> Shared<T> {
    unsafe fn new(ptr: *const T) -> Self {
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


impl<T> RawLink<T> {
    fn none() -> Self {
        RawLink(None)
    }

    fn some(link: &Node<T>) -> Self {
        RawLink(unsafe { Some(Shared::new(mem::transmute(link))) })
    }

    fn from_link(link: &Link<T>) -> Self {
        RawLink(link.as_ref().map(|bnode| unsafe { Shared::new(mem::transmute(&bnode)) }))
    }

    unsafe fn resolve_mut<'a>(&mut self) -> Option<&'a mut Node<T>> {
        self.0.as_ref().map(|p| &mut ***p)
    }
}


fn link_no_prev<T>(mut next: Box<Node<T>>) -> Link<T> {
    next.prev = RawLink::none();
    Some(next)
}
