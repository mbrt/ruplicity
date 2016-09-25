use std::io::{self, Read};
use std::path::{Path, PathBuf};
use tar::Archive;

use ::not_found;
use read::cache::BlockCache;
use read::block::BLOCK_SIZE;
use signatures::EntryId;

const NUM_READAHEAD_SNAP: usize = 10;
const NUM_READAHEAD_DIFF: usize = 6;


pub trait BlockStream: Read {
    fn seek_to_block(&mut self, n: usize) -> io::Result<()>;
}

pub trait Resources {
    fn snapshot_cache(&self) -> &BlockCache;
    fn signature_cache(&self) -> &BlockCache;
    fn volume<'a>(&'a self, n: usize) -> io::Result<Option<Archive<Box<Read + 'a>>>>;
    fn volume_of_block(&self, n: usize) -> Option<usize>;
}

pub struct NullStream;

pub struct SnapshotStream<'a> {
    res: &'a Resources,
    path: PathBuf,
    entry_id: EntryId,
    max_block: usize,
    curr_block: usize,
    buf: Box<[u8]>,
}


impl BlockStream for NullStream {
    fn seek_to_block(&mut self, n: usize) -> io::Result<()> {
        if n == 0 {
            Ok(())
        } else {
            Err(not_found("requested block > 0 for null stream"))
        }
    }
}

impl Read for NullStream {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}


impl<'a> SnapshotStream<'a> {
    pub fn new<P: AsRef<Path>>(resources: &'a Resources,
                               path: P,
                               entry_id: EntryId,
                               max_block: usize)
                               -> Self {
        SnapshotStream {
            res: resources,
            path: path.as_ref().to_owned(),
            entry_id: entry_id,
            max_block: max_block,
            curr_block: 0,
            buf: Box::new([0; BLOCK_SIZE]),
        }
    }
}

impl<'a> BlockStream for SnapshotStream<'a> {
    fn seek_to_block(&mut self, n: usize) -> io::Result<()> {
        if n > self.max_block {
            Err(not_found(format!("volume not found for block #{}", n)))
        } else {
            self.curr_block = n;
            Ok(())
        }
    }
}

impl<'a> Read for SnapshotStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        assert!(buf.len() >= BLOCK_SIZE); // we don't want buffering here
        if self.curr_block > self.max_block {
            return Ok(0); // eof
        }
        let vol_num = match self.res.volume_of_block(self.curr_block) {
            Some(n) => n,
            None => {
                return Err(not_found(format!("volume not found for block #{}", self.curr_block)));
            }
        };
        let mut archive = match try!(self.res.volume(vol_num)) {
            Some(a) => a,
            None => {
                return Err(not_found(format!("cannot open volume #{} for block #{}",
                                             vol_num,
                                             self.curr_block)));
            }
        };
        let cache = self.res.snapshot_cache();

        // read the current block and some additional ones
        let mut n_found = 0;
        let mut path = self.path.clone();
        path.push(self.curr_block.to_string());
        for entry in try!(archive.entries()) {
            let mut entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    if n_found > 0 {
                        // make sure to not get curr_block out of sync: safer and simpler to
                        // terminate
                        break;
                    } else {
                        // best effort: trying to overcome bad entries if we haven't found our one
                        continue;
                    }
                }
            };
            {
                let entry_path = match entry.path() {
                    Ok(e) => e,
                    Err(_) => {
                        // invalid path
                        continue;
                    }
                };
                if n_found > NUM_READAHEAD_SNAP {
                    break;
                } else if n_found == 0 {
                    // still need to find the first entry
                    if &entry_path < &path {
                        // the current path is still behind
                        continue;
                    } else if &entry_path > &path {
                        // we haven't found the path
                        break;
                    }
                }
                // check if the path is still the one expected, otherwise break
                if &entry_path != &path {
                    break;
                }
            }

            // we need to read this entry
            let block_id = (self.entry_id, self.curr_block);
            if n_found == 0 {
                // this entry is the one we are interested in
                let len = try!(entry.read(&mut self.buf));
                buf.copy_from_slice(&self.buf[..len]);
                cache.write(block_id, &self.buf[..len]);
            } else if !cache.cached(block_id) {
                // this is an entry to possibly cache
                let len = match entry.read(&mut self.buf) {
                    Ok(l) => l,
                    Err(_) => {
                        // invalid entry; don't care, since we already have what we need
                        break;
                    }
                };
                cache.write(block_id, &self.buf[..len]);
            }

            self.curr_block += 1;
            n_found += 1;
            path.pop();
            path.push(self.curr_block.to_string());
        }

        if n_found > 0 {
            Ok(BLOCK_SIZE)
        } else {
            Err(not_found(format!("block #{} not found in volume #{}",
                                  self.curr_block,
                                  vol_num)))
        }
    }
}
