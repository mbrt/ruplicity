use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::rc::{Rc, Weak};

use read::ptr::Shared;
use read::unslist::{self, UnsafeList};
use signatures::EntryId;


pub type BlockId = (EntryId, usize);

pub struct BlockCache {
    data: RefCell<CacheData>,
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
    max_blocks: usize,
}

struct Block {
    data: [u8; BLOCK_SIZE],
    len: usize,
    ref_count: Option<Weak<BlockRefCount>>,
}

// struct used to be shared among references to the same block
// when goes out of scope it triggers free of the related block in the cache
struct BlockRefCount {
    id: BlockId,
    cache: Shared<BlockCache>,
}

const BLOCK_SIZE: usize = 64 * 1024;

type BlockNode = unslist::Node<Block>;


impl BlockCache {
    pub fn new(max_blocks: usize) -> Self {
        BlockCache { data: RefCell::new(CacheData::new(max_blocks)) }
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
        self.data.borrow_mut().free_block(id);
    }
}


impl CacheData {
    fn new(max_blocks: usize) -> Self {
        CacheData {
            index: HashMap::new(),
            blocks: UnsafeList::new(),
            first_free: None,
            max_blocks: max_blocks,
        }
    }

    fn free_block(&mut self, id: BlockId) {
        let num_blocks = self.index.len();
        let remove_node = {
            let mut node_ptr = *self.index.get_mut(&id).unwrap();
            // if max cache size has been passed, free the block
            if num_blocks > self.max_blocks {
                unsafe {
                    self.blocks.remove(node_ptr);
                }
                true
            } else {
                // otherwise free memory for ref count,
                // move it at the end of the list
                let node = unsafe { node_ptr.resolve_mut() };
                debug_assert!(node.ref_count.as_ref().map_or(true, |rc| rc.upgrade().is_none()));
                node.ref_count = None;
                unsafe {
                    self.blocks.move_to_back(node_ptr);
                }
                false
            }
        };
        if remove_node {
            self.index.remove(&id);
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
    fn drop(&mut self) {
        let cache = unsafe { &**self.cache };
        cache.free_block(self.id);
    }
}


unsafe fn resolve_node(ptr: &Shared<BlockNode>) -> &BlockNode {
    &***ptr
}

unsafe fn resolve_node_mut(ptr: &mut Shared<BlockNode>) -> &mut BlockNode {
    &mut ***ptr
}
