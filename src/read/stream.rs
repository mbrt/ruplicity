use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::{self, FromStr};
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
    res: Box<Resources + 'a>,
    path: PathBuf,
    entry_id: EntryId,
    max_block: usize,
    curr_block: usize,
    buf: Box<[u8]>,
}


#[derive(Debug, Eq, PartialEq)]
struct BlockPath<'a> {
    entry_path: &'a [u8],
    block_type: BlockType,
}

#[derive(Debug, Eq, PartialEq)]
enum BlockType {
    MultivolSignature(usize), // include the block number
    MultivolSnapshot(usize), // include the block number
    Signature,
    Snapshot,
    Deleted,
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
    pub fn new<P: AsRef<Path>>(resources: Box<Resources + 'a>,
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

fn parse_block_path(path: &[u8]) -> Option<BlockPath> {
    // split the path in (first directory, the remaining path)
    // the first is the type, the remaining is the real path
    let pos = try_opt!(path.iter().cloned().position(|b| b == b'/'));
    let (pfirst, raw_real) = path.split_at(pos);
    let (p, t) = match pfirst {
        b"deleted" => (raw_real, BlockType::Deleted),
        b"signature" => (raw_real, BlockType::Signature),
        b"snapshot" => (raw_real, BlockType::Snapshot),
        b"multivol_signature" => {
            let (p, n) = try_opt!(strip_block_num(raw_real));
            (p, BlockType::MultivolSignature(n))
        }
        b"multivol_snapshot" => {
            let (p, n) = try_opt!(strip_block_num(raw_real));
            (p, BlockType::MultivolSnapshot(n))
        }
        _ => {
            return None;
        }
    };
    Some(BlockPath {
        entry_path: strip_trailing_slash(p),
        block_type: t,
    })
}

fn strip_trailing_slash(path: &[u8]) -> &[u8] {
    // safely assumes that the path starts with a slash
    assert!(path[0] == b'/');
    match path.last().cloned() {
        Some(b'/') if path.len() > 1 => &path[1..path.len() - 1],
        _ => &path[1..],
    }
}

// Removes from a path the last element, if it's an unsigned number, and return them.
fn strip_block_num(path: &[u8]) -> Option<(&[u8], usize)> {
    let pos = try_opt!(path.iter().cloned().rposition(|b| b == b'/'));
    let (p, num_b) = path.split_at(pos + 1);
    let num_str = try_opt!(str::from_utf8(num_b).ok());
    let num = try_opt!(usize::from_str(&num_str).ok());
    Some((p, num))
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn path_parse_block() {
        use super::{BlockPath, BlockType, parse_block_path};

        fn block_path(e: &[u8], t: BlockType) -> BlockPath {
            BlockPath {
                entry_path: e,
                block_type: t,
            }
        }

        assert_eq!(parse_block_path(b"deleted/foo"),
                   Some(block_path(b"foo", BlockType::Deleted)));
        assert_eq!(parse_block_path(b"signature/foo"),
                   Some(block_path(b"foo", BlockType::Signature)));
        assert_eq!(parse_block_path(b"snapshot/foo/"),
                   Some(block_path(b"foo", BlockType::Snapshot)));
        assert_eq!(parse_block_path(b"multivol_signature/foo/5"),
                   Some(block_path(b"foo", BlockType::MultivolSignature(5))));
        assert_eq!(parse_block_path(b"multivol_snapshot/foo/bar/5"),
                   Some(block_path(b"foo/bar", BlockType::MultivolSnapshot(5))));
        assert_eq!(parse_block_path(b"multivol_snapshot/foo/bar/b"), None);
        assert_eq!(parse_block_path(b"bla"), None);
        assert_eq!(parse_block_path(b"deleted/foo/5"),
                   Some(block_path(b"foo/5", BlockType::Deleted)));
    }

    #[test]
    fn path_parse_block_num() {
        use super::strip_block_num;

        assert_eq!(strip_block_num(b"my/long/path/3"),
                   Some((&b"my/long/path/"[..], 3)));
        assert_eq!(strip_block_num(b"/3"), Some((&b"/"[..], 3)));
        assert_eq!(strip_block_num(b"my/long/path/"), None);
        assert_eq!(strip_block_num(b"3"), None);
        assert_eq!(strip_block_num(b"/"), None);
        assert_eq!(strip_block_num(b""), None);
    }

    #[test]
    fn path_parse_trailing_slash() {
        use super::strip_trailing_slash;

        assert_eq!(strip_trailing_slash(b"/my/long/path/"),
                   &b"my/long/path"[..]);
        assert_eq!(strip_trailing_slash(b"/my/long/path"), &b"my/long/path"[..]);
        assert_eq!(strip_trailing_slash(b"///"), &b"/"[..]);
        assert_eq!(strip_trailing_slash(b"//"), &b""[..]);
        assert_eq!(strip_trailing_slash(b"/"), &b""[..]);
    }
}
