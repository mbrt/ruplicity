use std::ops::{Deref, DerefMut};
use std::mem;

use read::ptr::Shared;


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


impl<T> UnsafeList<T> {
    pub fn new() -> Self {
        UnsafeList {
            length: 0,
            head: None,
            tail: RawLink::none(),
        }
    }

    pub fn push_front(&mut self, val: T) -> &Node<T> {
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
        self.head.as_ref().unwrap()
    }

    pub fn push_back(&mut self, val: T) -> &Node<T> {
        match unsafe { self.tail.resolve_mut() } {
            None => self.push_front(val),
            Some(tail) => {
                let new_tail = Box::new(Node::new(val));
                tail.set_next(new_tail);
                self.tail = RawLink::from_link(&mut tail.next);
                self.length += 1;
                tail
            }
        }
    }

    pub unsafe fn remove(&mut self, node: &mut Node<T>) {
        unimplemented!()
    }

    pub unsafe fn move_to_end(&mut self, node: &mut Node<T>) {
        unimplemented!()
    }
}


impl<T> Node<T> {
    pub fn next(&self) -> Option<&Node<T>> {
        self.next.as_ref().map(|bnode| &**bnode)
    }

    pub fn prev(&self) -> Option<&Node<T>> {
        unsafe { self.prev.resolve() }
    }

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

impl<T> Deref for Node<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for Node<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
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

    unsafe fn resolve<'a>(&self) -> Option<&'a Node<T>> {
        self.0.as_ref().map(|p| &***p)
    }

    unsafe fn resolve_mut<'a>(&mut self) -> Option<&'a mut Node<T>> {
        self.0.as_ref().map(|p| &mut ***p)
    }
}


fn link_no_prev<T>(mut next: Box<Node<T>>) -> Link<T> {
    next.prev = RawLink::none();
    Some(next)
}
