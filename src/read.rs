use std::cmp::Ordering;
use std::io::{self, Read};

use tar;

use collections::BackupSet;
use manifest::Manifest;


pub struct MultiBlockIter<'e, 'p, R: Read + 'e> {
    archive: tar::Entries<'e, R>,
    path: &'p [u8],
}

pub struct MultiReader<R, I> {
    curr: Option<R>,
    iter: I,
}

pub type MultiBlockReader<'e, 'p, R> = MultiReader<tar::Entry<'e, R>, MultiBlockIter<'e, 'p, R>>;


impl<'e, 'p, R> MultiBlockIter<'e, 'p, R> where R: Read + 'e
{
    pub fn new(vol: tar::Entries<'e, R>, path: &'p [u8]) -> MultiBlockIter<'e, 'p, R> {
        MultiBlockIter {
            archive: vol,
            path: path,
        }
    }
}

impl<'e, 'p, R> Iterator for MultiBlockIter<'e, 'p, R> where R: Read + 'e
{
    type Item = tar::Entry<'e, R>;

    fn next(&mut self) -> Option<Self::Item> {
        for entry in &mut self.archive {
            let entry = try_opt!(entry.ok());
            let cmp = {
                let epath = entry.path_bytes();
                let path = &self.path;
                extract_path(epath.as_ref()).as_ref().map_or(Ordering::Less, |p| p.cmp(path))
            };
            match cmp {
                Ordering::Equal => {
                    return Some(entry);
                }
                Ordering::Greater => {
                    return None;
                }
                // continue the iteration
                Ordering::Less => (),
            }
        }
        None
    }
}


impl<R, I> MultiReader<R, I>
    where R: Read,
          I: Iterator<Item = R>
{
    pub fn new(mut iter: I) -> Self {
        let first = iter.next();
        MultiReader {
            curr: first,
            iter: iter,
        }
    }
}

impl<R, I> Read for MultiReader<R, I>
    where R: Read,
          I: Iterator<Item = R>
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.curr {
                Some(ref mut curr) => {
                    let len = try!(curr.read(buf));
                    if len > 0 {
                        return Ok(len);
                    }
                },
                None => {
                    return Ok(0);
                }
            }
            self.curr = self.iter.next();
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

pub fn multiblock_reader<'e, 'p, R: Read + 'e>(entries: tar::Entries<'e, R>,
                                               path: &'p [u8])
                                               -> MultiBlockReader<'e, 'p, R> {
    MultiReader::new(MultiBlockIter::new(entries, path))
}


// extract the real path from a raw path
//
// allowed:
// multivol_diff/my/path/num -> my/path
// snapshot/my/path -> my/path
//
// and similar
fn extract_path(raw_path: &[u8]) -> Option<&[u8]> {
    let pos = try_opt!(raw_path.iter().cloned().position(|b| b == b'/'));
    let (pfirst, raw_real) = raw_path.split_at(pos);
    if raw_real.len() < 2 {
        return None;
    }
    match pfirst {
        b"multivol_diff" | b"multivol_snapshot" => {
            match raw_real.iter().cloned().rposition(|b| b == b'/') {
                Some(pos) if pos > 1 => Some(&raw_real[1..pos]),
                _ => None,
            }
        }
        b"deleted" | b"diff" | b"snapshot" => Some(&raw_real[1..]),
        _ => None,
    }
}
