use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read};
use std::mem;

use read::ptr::Shared;
use read::unslist::{self, UnsafeList};


pub type BlockId = (usize, u8);

pub struct BlockCache {
    data: RefCell<CacheData>,
    max_blocks: usize,
}

pub struct BlockRef<'a> {
    id: BlockId,
    block: &'a [u8],
    cache: &'a BlockCache,
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
}

const BLOCK_SIZE: usize = 64 * 1024;

type BlockNode = unslist::Node<BlockCache>;


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
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.data[0..self.len]
    }
}
