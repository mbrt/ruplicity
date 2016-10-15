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
use ::ResourceCache;
use backend::Backend;
use collections::BackupChain;
use manifest::{Manifest, ManifestChain};
use rawpath::RawPath;
use read::block::{BLOCK_SIZE, BlockId};
use read::cache::BlockCache;
use read::stream::BlockStream;
use signatures::{Chain as SigChain, DiffType, EntryId};


pub struct Entry<'a, B: 'a> {
    res: ResourceProxy<'a, B>,
    buf: Box<[u8]>,
    len: usize,
    pos: usize,
    id: BlockId,
    stream: Option<Box<BlockStream + 'a>>,
}

pub struct BlockProvider {
    chain_id: usize,
    dcache: BlockCache,
    scache: BlockCache,
}


// sticks together block provider and top-level resources
struct ResourceProxy<'a, B: 'a> {
    provider: &'a BlockProvider,
    res: &'a ResourceCache<Backend = B>,
}

// Provides resources only for a specific entry
struct EntryResourceProxy<'a, B: 'a> {
    provider: &'a BlockProvider,
    res: &'a ResourceCache<Backend = B>,
    entry: EntryId,
}


impl<'a, B: Backend> Entry<'a, B> {
    fn fill_block(&mut self) -> io::Result<()> {
        let optlen = self.res.provider.read_cached_block(self.id, &mut self.buf);
        if let Some(len) = optlen {
            // the block is in cache, return it
            self.len = len;
            return Ok(());
        }
        // otherwise we need to use our block stream
        if self.stream.is_none() {
            // not present, create it now
            self.stream = Some(try!(self.res.block_stream(self.id.0)));
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


impl BlockProvider {
    pub fn new(chain_id: usize, cache_size: usize) -> Self {
        BlockProvider {
            chain_id: chain_id,
            dcache: BlockCache::new((cache_size as f64 * 0.3) as usize),
            scache: BlockCache::new((cache_size as f64 * 0.7) as usize),
        }
    }
}

impl BlockProvider {
    pub fn read<'a, B>(&'a self,
                       manifests: &'a ManifestChain,
                       bchain: &'a BackupChain,
                       sigchain: &'a SigChain,
                       backend: &'a B)
                       -> Option<Entry<'a, B>>
        where B: Backend + 'a
    {
        unimplemented!()
    }

    fn read_with_rescache<'a, B>(&'a self,
                                 res: &'a ResourceCache<Backend = B>,
                                 entry: EntryId)
                                 -> Option<Entry<'a, B>>
        where B: Backend + 'a
    {
        Some(Entry {
            res: ResourceProxy {
                provider: &self,
                res: res,
            },
            buf: vec![0; BLOCK_SIZE].into_boxed_slice(),
            len: 0,
            pos: 0,
            id: (entry, 0),
            stream: None,
        })
    }

    fn read_cached_block(&self, id: BlockId, buf: &mut [u8]) -> Option<usize> {
        self.scache.read(id, buf)
    }
}


impl<'a, B: Backend + 'a> ResourceProxy<'a, B> {
    fn block_stream(&self, entry: EntryId) -> io::Result<Box<BlockStream + 'a>> {
        let chain_id = self.provider.chain_id;
        let sig_chain = try!(self.res._signature_chain(chain_id));
        let sig_entry = match sig_chain.as_ref() {
            Some(sig) => sig.entry(entry),
            None => {
                return Err(not_found(format!("missing signature chain #{}", chain_id)));
            }
        };
        let path = sig_entry.path_bytes();
        let res = Box::new(EntryResourceProxy {
            provider: self.provider,
            res: self.res,
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
}


impl<'a, B: Backend + 'a> stream::Resources for EntryResourceProxy<'a, B> {
    fn snapshot_cache(&self) -> &BlockCache {
        &self.provider.scache
    }

    fn signature_cache(&self) -> &BlockCache {
        &self.provider.dcache
    }

    fn volume<'b>(&'b self, volnum: usize) -> io::Result<Option<Archive<Box<Read + 'b>>>> {
        let chain_id = self.provider.chain_id;
        let back = match self.res._collections().backup_chains().nth(chain_id) {
            Some(b) => b,
            None => {
                return Err(not_found(format!("backup chain #{} not found", chain_id)));
            }
        };
        let snapnum = self.entry.1 as usize;
        let backup_set = match back.nth_set(snapnum) {
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

        let rawfile = try!(self.res._backend().open_file(vol_path));
        let result: Box<Read + 'a> = if backup_set.is_compressed() {
            Box::new(try!(GzDecoder::new(rawfile)))
        } else {
            Box::new(rawfile)
        };

        Ok(Some(Archive::new(result)))
    }

    fn volume_of_block(&self, n: usize) -> io::Result<Option<usize>> {
        let snapnum = self.entry.1 as usize;
        let chain_id = self.provider.chain_id;
        let sig_chain = try!(self.res._signature_chain(chain_id));
        let entry = match sig_chain.as_ref() {
            Some(sig) => sig.entry(self.entry),
            None => {
                return Err(not_found(format!("missing signature chain #{}", chain_id)));
            }
        };
        let back = match self.res._collections().backup_chains().nth(chain_id) {
            Some(b) => b,
            None => {
                return Err(not_found(format!("backup chain #{} not found", chain_id)));
            }
        };
        let backup_set = match back.nth_set(snapnum) {
            Some(s) => s,
            None => {
                return Err(not_found(format!("backup set #{} not found", snapnum)));
            }
        };
        let manifest = match self.res._manifest(snapnum, backup_set.manifest_path()) {
            Ok(m) => m,
            Err(e) => {
                return Err(other(e));
            }
        };
        Ok(manifest.as_ref().and_then(|m| m.volume_of_block(entry.path_bytes(), n)))
    }
}
