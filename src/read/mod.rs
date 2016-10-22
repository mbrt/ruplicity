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
use manifest::Manifest;
use rawpath::RawPath;
use read::block::{BLOCK_SIZE, BlockId};
use read::cache::BlockCache;
use read::stream::BlockStream;
use signatures::{Chain as SigChain, DiffType, EntryId};


pub trait Resources {
    type Backend: Backend;
    type BackendRef: AsRef<Self::Backend>;
    type BackupChainRef: AsRef<BackupChain>;
    type SignatureRef: AsRef<SigChain>;
    type ManifestRef: AsRef<Manifest>;

    fn backend(&self) -> Self::BackendRef;
    fn backup_chain(&self) -> io::Result<Option<Self::BackupChainRef>>;
    fn signature_chain(&self) -> io::Result<Option<Self::SignatureRef>>;
    fn manifest(&self, id: usize) -> io::Result<Option<Self::ManifestRef>>;
}

pub struct Entry<'a, R: 'a> {
    provider: &'a BlockProvider<R>,
    buf: Box<[u8]>,
    len: usize,
    pos: usize,
    id: BlockId,
    stream: Option<Box<BlockStream + 'a>>,
}

pub struct BlockProvider<R> {
    res: R,
    dcache: BlockCache,
    scache: BlockCache,
}


// Provides resources only for a specific entry
struct EntryResourceProxy<'a, R: 'a> {
    provider: &'a BlockProvider<R>,
    entry: EntryId,
}


impl<'a, R: Resources> Entry<'a, R> {
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

impl<'a, R: Resources> Read for Entry<'a, R> {
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

impl<'a, R: Resources> BufRead for Entry<'a, R> {
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


impl<R: Resources> BlockProvider<R> {
    pub fn new(res: R, cache_size: usize) -> Self {
        BlockProvider {
            res: res,
            dcache: BlockCache::new((cache_size as f64 * 0.3) as usize),
            scache: BlockCache::new((cache_size as f64 * 0.7) as usize),
        }
    }
}

impl<R: Resources> BlockProvider<R> {
    pub fn read(&self, entry: EntryId) -> Option<Entry<R>> {
        Some(Entry {
            provider: &self,
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

    fn block_stream<'a>(&'a self, entry: EntryId) -> io::Result<Box<BlockStream + 'a>> {
        let sig_chain = try!(self.res.signature_chain());
        let sig_entry = match sig_chain.as_ref() {
            Some(sig) => sig.as_ref().entry(entry),
            None => {
                return Err(not_found("missing signature chain"));
            }
        };
        let path = sig_entry.path_bytes();
        let res = Box::new(EntryResourceProxy {
            provider: &self,
            entry: entry,
        });
        if let Some((_, max)) = sig_entry.size_hint() {
            if max == 0 {
                // optimize the empty file
                return Ok(Box::new(stream::NullStream));
            }
        }
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


impl<'a, R: Resources + 'a> stream::Resources for EntryResourceProxy<'a, R> {
    fn snapshot_cache(&self) -> &BlockCache {
        &self.provider.scache
    }

    fn signature_cache(&self) -> &BlockCache {
        &self.provider.dcache
    }

    fn volume<'b>(&'b self, volnum: usize) -> io::Result<Option<Archive<Box<Read + 'b>>>> {
        let back = match try!(self.provider.res.backup_chain()) {
            Some(b) => b,
            None => {
                return Err(not_found("backup chain not found"));
            }
        };
        let snapnum = self.entry.1 as usize;
        let backup_set = match back.as_ref().nth_set(snapnum) {
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

        let rawfile = try!(self.provider.res.backend().as_ref().open_file(vol_path));
        let result: Box<Read + 'a> = if backup_set.is_compressed() {
            Box::new(try!(GzDecoder::new(rawfile)))
        } else {
            Box::new(rawfile)
        };

        Ok(Some(Archive::new(result)))
    }

    fn volume_of_block(&self, n: usize) -> io::Result<Option<usize>> {
        let snapnum = self.entry.1 as usize;
        let sig_chain = try!(self.provider.res.signature_chain());
        let entry = match sig_chain.as_ref() {
            Some(sig) => sig.as_ref().entry(self.entry),
            None => {
                return Err(not_found("missing signature chain"));
            }
        };
        let manifest = match self.provider.res.manifest(snapnum) {
            Ok(m) => m,
            Err(e) => {
                return Err(other(e));
            }
        };
        Ok(manifest.as_ref().and_then(|m| m.as_ref().volume_of_block(entry.path_bytes(), n)))
    }
}
