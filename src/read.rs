use std::io::{self, Read};

use collections::BackupSet;
use manifest::Manifest;


pub struct MultiBlockReader<R, I> {
    curr: R,
    iter: I,
}


impl<R, I> Read for MultiBlockReader<R, I>
    where R: Read,
          I: Iterator<Item = R>
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let len = try!(self.curr.read(buf));
            if len > 0 {
                return Ok(len);
            }
            match self.iter.next() {
                Some(r) => {
                    self.curr = r;
                }
                None => {
                    return Ok(0);
                }
            }
        }
    }
}


pub fn volume_path_of_entry<'a>(chain: &'a BackupSet,
                                manifest: &Manifest,
                                entry_path: &[u8])
                                -> Option<&'a str> {
    let vol_num = try_opt!(manifest.first_volume_of_path(entry_path));
    chain.volume_path(vol_num)
}
