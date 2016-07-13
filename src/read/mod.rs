// mod iter;
#[allow(dead_code)]
mod block;
#[allow(dead_code)]
mod cache;
#[allow(dead_code)]
mod volume;

use self::cache::BlockCache;
use signatures::{Chain, EntryId};


pub struct Entry {}

struct BlockProvider {
    sig: Chain,
    dcache: BlockCache,
    scache: BlockCache,
}

impl BlockProvider {
    pub fn new(sigchain: Chain, cache_size: usize) -> Self {
        BlockProvider {
            sig: sigchain,
            dcache: BlockCache::new((cache_size as f64 * 0.4) as usize),
            scache: BlockCache::new((cache_size as f64 * 0.6) as usize),
        }
    }

    pub fn signature_chain(&self) -> &Chain {
        &self.sig
    }

    pub fn read(&self, entry: EntryId) -> Entry {
        unimplemented!()
    }
}
