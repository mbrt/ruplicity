use std::io::{self, Read};
use ::not_found;

use read::cache::BlockCache;
use read::block::BLOCK_SIZE;


pub trait BlockStream: Read {
    fn seek_to_block(&mut self, n: usize) -> io::Result<()>;
}

pub trait Resources {
    fn cache(&self) -> &BlockCache;
    fn volume_of_block<'a>(&'a self, n: usize) -> io::Result<Option<Box<Read + 'a>>>;
}

pub struct NullStream;

pub struct SnapshotStream<'a> {
    curr_block: usize,
    res: &'a Resources,
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
    pub fn new(resources: &'a Resources) -> Self {
        SnapshotStream {
            curr_block: 0,
            res: resources,
        }
    }
}

impl<'a> BlockStream for SnapshotStream<'a> {
    fn seek_to_block(&mut self, n: usize) -> io::Result<()> {
        self.curr_block = n;
        Ok(())
    }
}

impl<'a> Read for SnapshotStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        assert!(buf.len() >= BLOCK_SIZE); // we don't want buffering here
        match try!(self.res.volume_of_block(self.curr_block)) {
            Some(mut vol) => vol.read(buf),
            None => Ok(0), // end of blocks
        }
    }
}
