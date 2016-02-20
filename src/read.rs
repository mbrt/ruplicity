use std::cmp::Ordering;
use std::io::{self, Read};

use tar;

use collections::BackupSet;
use manifest::Manifest;


pub struct MultiBlockIter<'a, R: Read + 'a> {
    archive: tar::Entries<'a, R>,
    path: &'a [u8],
}

pub struct MultiReader<R, I> {
    curr: R,
    iter: I,
}


impl<'a, R: Read + 'a> MultiBlockIter<'a, R> {
    pub fn new(vol: tar::Entries<'a, R>, path: &'a [u8]) -> MultiBlockIter<'a, R> {
        MultiBlockIter {
            archive: vol,
            path: path,
        }
    }
}

impl<'a, R: Read + 'a> Iterator for MultiBlockIter<'a, R> {
    type Item = tar::Entry<'a, R>;

    fn next(&mut self) -> Option<Self::Item> {
        for entry in &mut self.archive {
            let entry = try_opt!(entry.ok());
            match entry.path_bytes().as_ref().cmp(self.path) {
                Ordering::Equal => {
                    return Some(entry);
                }
                Ordering::Greater => {
                    if entry.path_bytes().starts_with(self.path) {
                        return Some(entry);
                    }
                }
                Ordering::Less => {
                    return None;
                }
            }
        }
        None
    }
}


impl<R, I> Read for MultiReader<R, I>
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
