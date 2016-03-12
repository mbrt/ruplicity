use std::cmp;
use std::io::{self, Read};
use std::sync::RwLock;
use linked_hash_map::LinkedHashMap;

use signatures::EntryId;


pub type BlockId = (EntryId, usize);

pub struct BlockCache {
    // map from index to block
    // all blocks must be indexed, even unused
    index: RwLock<LinkedHashMap<BlockId, Block>>,
    max_blocks: usize,
}

struct Block(Vec<u8>);


const BLOCK_SIZE: usize = 64 * 1024;


impl BlockCache {
    pub fn new(max_blocks: usize) -> Self {
        BlockCache {
            index: RwLock::new(LinkedHashMap::new()),
            max_blocks: max_blocks,
        }
    }

    pub fn read(&self, id: BlockId, buffer: &mut [u8]) -> Option<usize> {
        {
            // first refresh the block if present, by using write lock
            if self.index.write().unwrap().get_refresh(&id).is_none() {
                return None;
            }
        }

        // then read by taking the read lock only
        let index = self.index.read().unwrap();
        match index.get(&id) {
            None => None, // this can be possible even with the refresh above
            Some(block) => block.as_slice().read(buffer).ok(),
        }
    }

    pub fn write(&self, id: BlockId, buffer: &[u8]) -> Option<usize> {
        let mut index = self.index.write().unwrap();
        if index.get(&id).is_some() {
            // already written by someone else, don't change
            return None;
        }

        if index.len() >= self.max_blocks && !index.is_empty() {
            // the cache is full, reuse the least used block
            let old_block = index.pop_front().unwrap().1;
            index.insert(id, old_block);
        } else {
            // we can add another block
            index.insert(id, Block::new());
        }
        let block = index.get_mut(&id).unwrap();
        block.write_max_block(buffer).ok()
    }
}


impl Block {
    fn new() -> Self {
        Block(Vec::with_capacity(BLOCK_SIZE))
    }

    fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }

    fn write_max_block(&mut self, buffer: &[u8]) -> io::Result<usize> {
        use std::io::Write;
        let buffer = &buffer[0..cmp::min(buffer.len(), BLOCK_SIZE)];
        self.0.write_all(buffer).map(|_| buffer.len())
    }
}
