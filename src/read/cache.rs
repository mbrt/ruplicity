use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read};
use std::mem;

pub type BlockId = (usize, u8);

const BLOCK_SIZE: usize = 64 * 1024;


pub struct BlockCache {
    // map from index to block
    // all blocks must be indexed, even unused
    index: RefCell<HashMap<BlockId, *const Block>>,
    // the list of all blocks
    // [0..first_free] -> used
    // [first_free..] -> not used, sorted by last usage (last used is last)
    blocks: RefCell<Vec<Box<Block>>>,
    max_blocks: usize,
    first_free: usize,
}

pub struct BlockRef<'a> {
    id: BlockId,
    block: &'a [u8],
    cache: &'a BlockCache,
}

struct Block {
    data: [u8; BLOCK_SIZE],
    len: usize,
}


impl BlockCache {
    pub fn new(max_blocks: usize) -> Self {
        BlockCache {
            index: RefCell::new(HashMap::new()),
            blocks: RefCell::new(Vec::new()),
            max_blocks: max_blocks,
            first_free: 0,
        }
    }

    pub fn block(&self, id: BlockId) -> Option<BlockRef> {
        self.index.borrow().get(&id).map(|block| {
            let bref: &Block = unsafe { mem::transmute(block) };
            BlockRef {
                id: id,
                block: bref.as_slice(),
                cache: self,
            }
        })
    }

    /// Returns a cached block or loads it with the given function.
    pub fn block_or_load_with<F>(&self, id: BlockId, f: F) -> io::Result<BlockRef>
        where F: FnMut(&mut [u8]) -> io::Result<usize>
    {
        if let Some(block) = self.index.borrow().get(&id) {
            let bref: &Block = unsafe { mem::transmute(block) };
            return Ok(BlockRef {
                id: id,
                block: bref.as_slice(),
                cache: self,
            });
        }

        // need to load the block
        let index = self.index.borrow_mut();
        let mut block = {
            if index.len() >= self.max_blocks && self.first_free < index.len() {
                // max cache size reached and some block is unused
            } else {
                // append a new block, because all blocks are used or the cache is not full
                let block = Box::new(Block::new());
            }
        };

        unimplemented!()
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
