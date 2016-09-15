use std::io::{self, Read};
use std::path::{Path, PathBuf};

use ::not_found;
use read::cache::BlockCache;
use read::block::BLOCK_SIZE;


pub trait BlockStream: Read {
    fn seek_to_block(&mut self, n: usize) -> io::Result<()>;
}

pub trait Resources {
    fn cache(&self) -> &BlockCache;
    fn volume<'a>(&'a self, n: usize) -> io::Result<Option<Box<Read + 'a>>>;
    // Returns the min and max block nums for the volume
    fn volume_blocks(&self, n: usize) -> (usize, usize);
    fn volume_of_block(&self, n: usize) -> Option<usize>;
}

pub struct NullStream;

pub struct SnapshotStream<'a> {
    res: &'a Resources,
    max_block: usize,
    path: PathBuf,
    curr_block: usize,
    curr_vol_num: usize,
    curr_vol: Option<Box<Read + 'a>>,
    curr_vol_boundaries: Option<(usize, usize)>,
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
                               max_block: usize,
                               volume: usize)
                               -> Self {
        SnapshotStream {
            res: resources,
            max_block: max_block,
            path: path.as_ref().to_owned(),
            curr_block: 0,
            curr_vol_num: volume,
            curr_vol: None,
            curr_vol_boundaries: None,
        }
    }
}

impl<'a> BlockStream for SnapshotStream<'a> {
    fn seek_to_block(&mut self, n: usize) -> io::Result<()> {
        if n == self.curr_block {
            return Ok(());
        }

        // use the cached volume boundaries, or compute them on demand
        let vol_boundaries = match self.curr_vol_boundaries {
            Some(b) => b,
            None => {
                let b = self.res.volume_blocks(self.curr_vol_num);
                self.curr_vol_boundaries = Some(b);
                b
            }
        };
        // we cannot reuse the volume if:
        // * we have to move backward
        // * the block is past the last block in the volume
        let reuse_vol = n >= self.curr_block && self.curr_block <= vol_boundaries.1;
        if !reuse_vol {
            // get rid of the current volume
            self.curr_vol = None;
            if n < vol_boundaries.0 || n > vol_boundaries.1 {
                // we are outside the volume range:
                // update the volume number and the boundaries
                self.curr_vol_num = match self.res.volume_of_block(n) {
                    Some(num) => num,
                    None => {
                        return Err(not_found(format!("volume not found for block #{}", n)));
                    }
                };
                self.curr_vol_boundaries = None;
            }
        }
        self.curr_block = n;
        Ok(())
    }
}

impl<'a> Read for SnapshotStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        assert!(buf.len() >= BLOCK_SIZE); // we don't want buffering here
        if self.curr_vol.is_none() {
            self.curr_vol = try!(self.res.volume(self.curr_vol_num));
            self.curr_vol_num += 1;
        }
        self.curr_block += 1;
        unimplemented!()
    }
}
