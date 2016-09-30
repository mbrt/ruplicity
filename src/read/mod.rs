// mod iter;
#[allow(dead_code)]
mod block;
#[allow(dead_code)]
mod cache;
#[allow(dead_code)]
mod stream;
#[allow(dead_code)]
mod volume;

use std::cmp;
use std::io::{self, BufRead, Read};
use std::path::Path;
use flate2::read::GzDecoder;
use tar::Archive;

use ::not_found;
use ::other;
use backend::Backend;
use collections::BackupChain;
use manifest::{Manifest, ManifestChain};
use rawpath::RawPath;
use read::block::{BLOCK_SIZE, BlockId};
use read::cache::BlockCache;
use read::stream::BlockStream;
use signatures::{Chain as SigChain, DiffType, EntryId};


pub struct Entry<'a, B: 'a> {
    provider: &'a BlockProvider<B>,
    buf: Box<[u8]>,
    len: usize,
    pos: usize,
    id: BlockId,
    stream: Option<Box<BlockStream + 'a>>,
}

pub struct BlockProvider<B> {
    manifests: ManifestChain,
    back: BackupChain,
    sig: SigChain,
    backend: B,
    dcache: BlockCache,
    scache: BlockCache,
}

/// wip
#[derive(Copy, Clone, Debug)]
pub enum StorageType {
    /// wip
    Deleted,
    /// wip
    MultivolSignature,
    /// wip
    MultivolSnapshot,
    /// wip
    Signature,
    /// wip
    Snapshot,
}


// Provides resources only for a specific entry
struct EntryResourceProxy<'a, B: 'a> {
    provider: &'a BlockProvider<B>,
    entry: EntryId,
}


impl<'a, B: Backend> Entry<'a, B> {
    fn fill_block(&mut self) -> io::Result<()> {
        let optlen = self.provider.read_cached_block(self.id, &mut self.buf);
        if let Some(len) = optlen {
            // the block is in cache, return it
            self.len = len;
            return Ok(());
        }
        // otherwise we need to use our block stream
        if self.stream.is_none() {
            // not present, create it now
            self.stream = Some(try!(self.provider.block_stream(self.id.0)));
        }
        let mut stream = self.stream.as_mut().unwrap();
        try!(stream.seek_to_block(self.id.1));
        self.len = try!(stream.read(&mut self.buf));
        self.pos = 0;
        Ok(())
    }
}

impl<'a, B: Backend> Read for Entry<'a, B> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.len > 0 {
            // we have buffered stuff... just copy as much as possible
            let len = cmp::min(self.len, buf.len());
            buf.copy_from_slice(&self.buf[self.pos..self.pos + len]);
            self.pos += len;
            self.len -= len;
            Ok(len)
        } else {
            // try to fill the block by using the provider
            try!(self.fill_block());
            self.id.1 += 1;
            if self.len > 0 {
                // recurse, now we are sure there's something buffered
                self.read(buf)
            } else {
                // end of the stream
                Ok(0)
            }
        }
    }
}

impl<'a, B: Backend> BufRead for Entry<'a, B> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.len == 0 {
            try!(self.fill_block());
        }
        Ok(&self.buf[self.pos..self.pos + self.len])
    }

    fn consume(&mut self, amt: usize) {
        let amt = cmp::min(amt, self.len);
        self.pos += amt;
        self.len -= amt;
    }
}


impl<B> BlockProvider<B> {
    pub fn new(manifests: ManifestChain,
               bchain: BackupChain,
               sigchain: SigChain,
               backend: B,
               cache_size: usize)
               -> Self {
        BlockProvider {
            manifests: manifests,
            back: bchain,
            sig: sigchain,
            backend: backend,
            dcache: BlockCache::new((cache_size as f64 * 0.3) as usize),
            scache: BlockCache::new((cache_size as f64 * 0.7) as usize),
        }
    }

    pub fn signature_chain(&self) -> &SigChain {
        &self.sig
    }
}

impl<B: Backend> BlockProvider<B> {
    pub fn read(&self, entry: EntryId) -> Option<Entry<B>> {
        Some(Entry {
            provider: &self,
            buf: Box::new([0; BLOCK_SIZE]),
            len: 0,
            pos: 0,
            id: (entry, 0),
            stream: None,
        })
    }

    fn read_cached_block(&self, id: BlockId, buf: &mut [u8]) -> Option<usize> {
        self.scache.read(id, buf)
    }

    fn block_stream<'a>(&'a self, entry: EntryId) -> io::Result<Box<BlockStream + 'a>> {
        let sig_entry = self.sig.entry(entry);
        let path = sig_entry.path_bytes();
        let res = Box::new(EntryResourceProxy {
            provider: &self,
            entry: entry,
        });
        match sig_entry.diff_type() {
            DiffType::Snapshot => {
                Ok(Box::new(stream::SnapshotStream::new(res,
                                                        RawPath::new(path).as_raw_path_buf(),
                                                        entry,
                                                        0)))
            }
            DiffType::Signature => unimplemented!(),
            _ => Ok(Box::new(stream::NullStream)),
        }
    }

    fn manifest_of_entry(&self, entryid: EntryId) -> Option<&Manifest> {
        let snapnum = entryid.1 as usize;
        let entry = self.sig.entry(entryid);
        self.manifests.get(snapnum)
    }

    fn volume_of_block(&self, id: BlockId) -> Option<usize> {
        let entry = self.sig.entry(id.0);
        self.manifest_of_entry(id.0).and_then(|m| m.volume_of_block(entry.path_bytes(), id.1))
    }

    fn volume<'a>(&'a self,
                  snapnum: usize,
                  volnum: usize)
                  -> io::Result<Option<Archive<Box<Read + 'a>>>> {
        let backup_set = match self.back.nth_set(snapnum) {
            Some(s) => s,
            None => {
                return Err(not_found(format!("backup set #{} not found", snapnum)));
            }
        };
        let vol_path = match backup_set.volume_path(volnum) {
            Some(p) => Path::new(p),
            None => {
                return Err(not_found(format!("no path for volume #{}", volnum)));
            }
        };
        if backup_set.is_encrypted() {
            return Err(other("encrypted backups are not supported"));
        }

        let rawfile = try!(self.backend.open_file(vol_path));
        let result: Box<Read + 'a> = if backup_set.is_compressed() {
            Box::new(try!(GzDecoder::new(rawfile)))
        } else {
            Box::new(rawfile)
        };

        Ok(Some(Archive::new(result)))
    }
}

impl<'a, B: Backend + 'a> stream::Resources for EntryResourceProxy<'a, B> {
    fn snapshot_cache(&self) -> &BlockCache {
        &self.provider.scache
    }

    fn signature_cache(&self) -> &BlockCache {
        &self.provider.dcache
    }

    fn volume<'b>(&'b self, n: usize) -> io::Result<Option<Archive<Box<Read + 'b>>>> {
        self.provider.volume(self.entry.1 as usize, n)
    }

    fn volume_of_block(&self, n: usize) -> Option<usize> {
        self.provider.volume_of_block(((self.entry), n))
    }
}
