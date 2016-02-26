use std::cell::RefCell;
use std::io::{self, Read};

pub type BlockId = (usize, u8);

const BLOCK_SIZE: usize = 64 * 1024;


pub struct BlockCache {
    free_list: Vec<Block>,
    used_blocks: Vec<Block>,
    size: usize,
    max_size: usize,
}

pub struct BlockRef<'a> {
    block: &'a [u8],
    cache: &'a BlockCache,
}

pub struct BlockRefMut<'a> {
    block: &'a mut [u8],
    size: &'a mut usize,
    cache: &'a BlockCache,
}

struct Block {
    data: [u8; BLOCK_SIZE],
    len: usize,
}


impl BlockCache {
    pub fn new(max_size: usize) -> Self {
        BlockCache {
            free_list: vec![],
            used_blocks: vec![],
            size: 0,
            max_size: max_size,
        }
    }

    pub fn block(&self, id: BlockId) -> Option<BlockRef> {
        unimplemented!()
    }

    /// Returns a cached block or a fresh one to be written
    pub fn cached_or_free_block(&self, id: BlockId) -> Result<BlockRef, BlockRefMut> {
        unimplemented!()
    }
}


impl<'a> BlockRefMut<'a> {
    pub fn read<R: Read>(&mut self, r: &mut R) -> io::Result<usize> {
        let mut size = 0;
        loop {
            let len = try!(r.read(&mut self.block[size..]));
            if len == 0 {
                break;
            }
            size += len;
        }
        *self.size = size;
        Ok(size)
    }
}

impl<'a> Into<BlockRef<'a>> for BlockRefMut<'a> {
    fn into(self) -> BlockRef<'a> {
        BlockRef {
            block: &self.block[0..*self.size],
            cache: self.cache,
        }
    }
}
