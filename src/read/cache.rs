use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read};
use std::mem;
use std::rc::{Rc, Weak};

use read::ptr::Shared;
use read::unslist::{self, UnsafeList};
use signatures::EntryId;


pub type BlockId = (EntryId, usize);

pub struct BlockCache {
    data: RefCell<CacheData>,
    max_blocks: usize,
}

pub struct BlockRef<'a> {
    block: &'a Block,
    ref_count: Rc<BlockRefCount>,
}


struct CacheData {
    // map from index to block
    // all blocks must be indexed, even unused
    index: HashMap<BlockId, Shared<BlockNode>>,
    // the list of all blocks
    // [0..first_free] -> used
    // [first_free..] -> not used, sorted by last usage (last used is last)
    blocks: UnsafeList<Block>,
    first_free: Option<Shared<BlockNode>>,
}

struct Block {
    data: [u8; BLOCK_SIZE],
    len: usize,
    ref_count: Option<Weak<BlockRefCount>>,
}

struct BlockRefCount {
    id: BlockId,
    cache: Shared<BlockCache>,
}

const BLOCK_SIZE: usize = 64 * 1024;

type BlockNode = unslist::Node<Block>;


impl BlockCache {
    pub fn new(max_blocks: usize) -> Self {
        BlockCache {
            data: RefCell::new(CacheData::new()),
            max_blocks: max_blocks,
        }
    }

    pub fn block(&self, id: BlockId) -> Option<BlockRef> {
        let data = self.data.borrow_mut();
        unimplemented!()
    }

    /// Returns a cached block or loads it with the given function.
    pub fn block_or_load_with<F>(&self, id: BlockId, f: F) -> io::Result<BlockRef>
        where F: FnMut(&mut [u8]) -> io::Result<usize>
    {
        unimplemented!()
    }

    fn free_block(&self, id: BlockId) {
        unimplemented!()
/*
        let mut data = self.data.borrow_mut();
        let num_blocks = data.index.len();
        let index = &mut data.index;
        let blocks = &mut data.blocks;
        let node = unsafe { resolve_node_mut(index.get_mut(&id).unwrap()) };
        // if max cache size has been passed, free the block
        if num_blocks > self.max_blocks {
            unsafe {
                blocks.remove(node);
            }
        } else {
            // otherwise free memory for ref count,
            // move it at the end of the list
            debug_assert!(node.ref_count.as_ref().map_or(true, |rc| rc.upgrade().is_none()));
            node.ref_count = None;
            unsafe {
                blocks.move_to_end(node);
            }
        }
*/
    }
}


impl CacheData {
    pub fn new() -> Self {
        CacheData {
            index: HashMap::new(),
            blocks: UnsafeList::new(),
            first_free: None,
        }
    }
}


impl Block {
    fn new() -> Self {
        Block {
            data: [0; BLOCK_SIZE],
            len: 0,
            ref_count: None,
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.data[0..self.len]
    }
}


impl Drop for BlockRefCount {
    fn drop(&mut self) {}
}


unsafe fn resolve_node(ptr: &Shared<BlockNode>) -> &BlockNode {
    &***ptr
}

unsafe fn resolve_node_mut(ptr: &mut Shared<BlockNode>) -> &mut BlockNode {
    &mut ***ptr
}
