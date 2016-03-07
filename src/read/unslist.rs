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

    pub fn push_front(&mut self, val: T) -> Shared<Node<T>> {
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
        unsafe { Shared::new(mem::transmute(self.head.as_mut().unwrap())) }
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.head.take().map(|mut front_node| {
            self.length -= 1;
            match front_node.next.take() {
                Some(node) => self.head = link_no_prev(node),
                None => self.tail = RawLink::none(),
            }
            front_node.value
        })
    }

    pub fn push_back(&mut self, val: T) -> Shared<Node<T>> {
        match unsafe { self.tail.resolve_mut() } {
            None => self.push_front(val),
            Some(tail) => {
                let new_tail = Box::new(Node::new(val));
                tail.set_next(new_tail);
                self.tail = RawLink::from_link(&mut tail.next);
                self.length += 1;
                unsafe { Shared::new(tail) }
            }
        }
    }

    pub fn pop_back(&mut self) -> Option<T> {
        unsafe {
            self.tail.resolve_mut().and_then(|tail| {
                self.length -= 1;
                self.tail = tail.prev;
                let opt_node = match tail.prev.resolve_mut() {
                    None => self.head.take(),
                    Some(tail_prev) => tail_prev.next.take(),
                };
                opt_node.map(|bnode| bnode.value)
            })
        }
    }

    /// Remove the given node.
    ///
    /// unsafe because:
    /// * the node can belong to another list
    /// * or it could be already freed
    pub unsafe fn remove(&mut self, node: Shared<Node<T>>) {
        self.move_to_back(node);
        self.pop_back();
    }

    /// Move the given node to the back.
    ///
    /// unsafe because:
    /// * the node can belong to another list
    /// * or it could be already freed
    pub unsafe fn move_to_back(&mut self, mut node: Shared<Node<T>>) {
        let node = node.resolve_mut();

        // take node out of its neighbors
        let node = match (node.prev.resolve_mut(), node.next.take()) {
            (_, None) => {
                // nothing to do, the node is already in the back
                return;
            }
            (None, Some(next_node)) => {
                // the node is the head
                let node = self.head.take().unwrap();
                self.head = link_no_prev(next_node);
                node
            }
            (Some(prev_node), Some(next_node)) => {
                // the node is in the middle
                let node = prev_node.next.take().unwrap();
                prev_node.set_next(next_node);
                node
            }
        };
        // move node at the end
        let tail = self.tail.resolve_mut().unwrap();
        tail.set_next(node);
        self.tail = RawLink::from_link(&mut tail.next);
    }
}

impl<T> Drop for UnsafeList<T> {
    fn drop(&mut self) {
        // Dissolve the linked_list in a loop.
        // Just dropping the list_head can lead to stack exhaustion
        // when length is >> 1_000_000
        while let Some(mut head) = self.head.take() {
            self.head = head.next.take();
        }
        self.length = 0;
        self.tail = RawLink::none();
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

    fn some(link: &mut Node<T>) -> Self {
        RawLink(unsafe { Some(Shared::new(mem::transmute(link))) })
    }

    fn from_link(link: &mut Link<T>) -> Self {
        RawLink(link.as_ref().map(|bnode| unsafe { Shared::new(mem::transmute(&*bnode)) }))
    }

    unsafe fn resolve<'a>(&self) -> Option<&'a Node<T>> {
        self.0.as_ref().map(|p| &***p)
    }

    unsafe fn resolve_mut<'a>(&mut self) -> Option<&'a mut Node<T>> {
        self.0.as_ref().map(|p| &mut ***p)
    }
}

impl<T> Copy for RawLink<T> {}

impl<T> Clone for RawLink<T> {
    fn clone(&self) -> Self {
        *self
    }
}


fn link_no_prev<T>(mut next: Box<Node<T>>) -> Link<T> {
    next.prev = RawLink::none();
    Some(next)
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn push_pop_front() {
        let mut list = UnsafeList::new();
        list.push_front(1);
        list.push_front(2);
        list.push_front(3);
        assert_eq!(list.pop_front(), Some(3));
        assert_eq!(list.pop_front(), Some(2));
        assert_eq!(list.pop_front(), Some(1));
        assert_eq!(list.pop_front(), None);
    }

    #[test]
    fn push_pop_back() {
        let mut list = UnsafeList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        assert_eq!(list.pop_back(), Some(3));
        assert_eq!(list.pop_back(), Some(2));
        assert_eq!(list.pop_back(), Some(1));
        assert_eq!(list.pop_back(), None);
    }
}
