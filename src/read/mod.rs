// mod iter;
#[allow(dead_code)]
mod block;
#[allow(dead_code)]
mod cache;
#[allow(dead_code)]
mod volume;

use std::io::{self, Read};
use std::cmp;

use self::cache::BlockCache;
use self::block::{BLOCK_SIZE, BlockId};
use collections::BackupChain;
use manifest::ManifestChain;
use signatures::{Chain, DiffType, Entry as SnapEntry, EntryId};


pub struct Entry<'a> {
    provider: &'a BlockProvider,
    buf: Box<[u8]>,
    len: usize,
    pos: usize,
    id: BlockId,
}

struct BlockProvider {
    manifests: ManifestChain,
    back: BackupChain,
    sig: Chain,
    dcache: BlockCache,
    scache: BlockCache,
}


impl<'a> Read for Entry<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.len > 0 {
            // we have buffered stuff... just copy as much as possible
            let len = cmp::min(self.len - self.pos, buf.len());
            buf[..len].copy_from_slice(&self.buf[self.pos..self.pos + len]);
            self.pos += len;
            self.len -= len;
            Ok(len)
        } else {
            // try to pick the next block
            self.id.1 += 1;
            self.len = try!(self.provider.read_block(self.id, &mut self.buf));
            if self.len > 0 {
                // recurse, now we are sure there's something buffered
                self.pos = 0;
                self.read(buf)
            } else {
                // end of the stream
                Ok(0)
            }
        }
    }
}


impl BlockProvider {
    pub fn new(manifests: ManifestChain,
               bchain: BackupChain,
               sigchain: Chain,
               cache_size: usize)
               -> Self {
        BlockProvider {
            manifests: manifests,
            back: bchain,
            sig: sigchain,
            dcache: BlockCache::new((cache_size as f64 * 0.4) as usize),
            scache: BlockCache::new((cache_size as f64 * 0.6) as usize),
        }
    }

    pub fn signature_chain(&self) -> &Chain {
        &self.sig
    }

    pub fn read(&self, entry: EntryId) -> Option<Entry> {
        Some(Entry {
            provider: &self,
            buf: Box::new([0; BLOCK_SIZE]),
            len: 0,
            pos: 0,
            id: (entry, 0),
        })
    }

    fn read_block(&self, id: BlockId, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(len) = self.dcache.read(id, buf) {
            // already cached block, let's return it
            return Ok(len);
        }

        // look for the volume containing that block
        let entry = self.sig.entry(id.0);
        let manifest = self.manifests.iter().nth((id.0).1 as usize).unwrap();
        let opt_volume = manifest.volume_of_block(entry.path_bytes(), id.1)
            .map(|n| manifest.volume(n));
        let volume = match opt_volume {
            Some(v) => v,
            None => {
                // no more blocks
                return Ok(0);
            }
        };

        // open the volume

        // determine the entry type
        match entry.diff_type() {
            DiffType::Snapshot => (),
            DiffType::Signature => (),
            _ => unreachable!(),
        };
        unimplemented!()
    }
}
