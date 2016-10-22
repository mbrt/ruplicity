use std::cmp::Ordering;
use std::io::{self, Read};
use std::str::{self, FromStr};
use tar::Archive;

use ::not_found;
use rawpath::{RawPath, RawPathBuf};
use read::cache::BlockCache;
use read::block::BLOCK_SIZE;
use signatures::EntryId;

const NUM_READAHEAD_SNAP: usize = 9;
const NUM_READAHEAD_DIFF: usize = 5;


pub trait BlockStream: Read {
    fn seek_to_block(&mut self, n: usize) -> io::Result<()>;
}

pub trait Resources {
    fn snapshot_cache(&self) -> &BlockCache;
    fn signature_cache(&self) -> &BlockCache;
    fn volume<'a>(&'a self, n: usize) -> io::Result<Option<Archive<Box<Read + 'a>>>>;
    fn volume_of_block(&self, n: usize) -> io::Result<Option<usize>>;
}

pub struct NullStream;

pub struct SnapshotStream<'a> {
    res: Box<Resources + 'a>,
    path: RawPathBuf,
    entry_id: EntryId,
    max_block: usize,
    curr_block: usize,
    buf: Box<[u8]>,
}


#[derive(Debug, Eq, PartialEq)]
struct BlockPath<'a> {
    entry_path: RawPath<'a>,
    block_type: BlockType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    pub fn new(resources: Box<Resources + 'a>,
               path: RawPathBuf,
               entry_id: EntryId,
               max_block: usize)
               -> Self {
        SnapshotStream {
            res: resources,
            path: path,
            entry_id: entry_id,
            max_block: max_block,
            curr_block: 0,
            buf: vec![0; BLOCK_SIZE].into_boxed_slice(),
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
        let vol_num = match try!(self.res.volume_of_block(self.curr_block)) {
            Some(n) => n,
            _ => {
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
        let mut res_len = 0;
        let path = self.path.as_raw_path();
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
                let path_bytes = entry.path_bytes();
                let raw_path = RawPath::new(path_bytes.as_ref());
                let block_path = match parse_block_path(raw_path.as_bytes()) {
                    Some(bp) => bp,
                    None => {
                        // invalid path
                        continue;
                    }
                };
                if n_found > NUM_READAHEAD_SNAP {
                    break;
                } else if n_found == 0 {
                    // still need to find the first entry
                    match block_path.entry_path.cmp(&path) {
                        Ordering::Less => {
                            // the current path is still behind
                            continue;
                        }
                        Ordering::Greater => {
                            // we haven't found the path
                            break;
                        }
                        Ordering::Equal => {
                            // check the block num
                            if block_path.block_type.block_num().unwrap_or(0) != self.curr_block {
                                continue;
                            }
                        }
                    }
                }
            }

            // we need to read this entry
            let block_id = (self.entry_id, self.curr_block);
            if n_found == 0 {
                // this entry is the one we are interested in
                let len = try!(read_all(&mut entry, &mut self.buf));
                buf[..len].copy_from_slice(&self.buf[..len]);
                cache.write(block_id, &self.buf[..len]);
                res_len = len;
            } else if !cache.cached(block_id) {
                // this is an entry we could possibly cache
                let len = match read_all(&mut entry, &mut self.buf) {
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
        }

        if n_found > 0 {
            Ok(res_len)
        } else {
            Err(not_found(format!("block #{} not found in volume #{}",
                                  self.curr_block,
                                  vol_num)))
        }
    }
}


impl BlockType {
    fn block_num(&self) -> Option<usize> {
        match *self {
            BlockType::MultivolSignature(n) |
            BlockType::MultivolSnapshot(n) => Some(n),
            _ => None,
        }
    }
}

fn read_all<R: Read>(r: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut tot_len = 0;
    loop {
        let len = try!(r.read(&mut buf[tot_len..]));
        if len == 0 {
            break;
        }
        tot_len += len;
    }
    Ok(tot_len)
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
            (p, BlockType::MultivolSignature(n - 1))
        }
        b"multivol_snapshot" => {
            let (p, n) = try_opt!(strip_block_num(raw_real));
            (p, BlockType::MultivolSnapshot(n - 1))
        }
        _ => {
            return None;
        }
    };
    Some(BlockPath {
        entry_path: RawPath::new(strip_trailing_slash(p)),
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
    use rawpath::RawPath;
    use read::block::BLOCK_SIZE;
    use read::cache::BlockCache;

    use std::fs::File;
    use std::io::{self, Read};
    use std::path::{Path, PathBuf};
    use flate2::read::GzDecoder;
    use tar::Archive;

    struct TestResources<'a> {
        snap_cache: &'a BlockCache,
        sig_cache: &'a BlockCache,
        vol_path: PathBuf,
        vol_num: usize,
    }

    impl<'a> Resources for TestResources<'a> {
        fn snapshot_cache(&self) -> &BlockCache {
            &self.snap_cache
        }

        fn signature_cache(&self) -> &BlockCache {
            &self.sig_cache
        }

        fn volume<'b>(&'b self, n: usize) -> io::Result<Option<Archive<Box<Read + 'b>>>> {
            if n != self.vol_num {
                Ok(None)
            } else {
                let file = try!(File::open(&self.vol_path));
                let dec = try!(GzDecoder::new(file));
                Ok(Some(Archive::new(Box::new(dec))))
            }
        }

        fn volume_of_block(&self, _: usize) -> io::Result<Option<usize>> {
            Ok(Some(self.vol_num))
        }
    }

    fn block_contents(vol_path: &Path, entry_path: &[u8]) -> Option<Vec<u8>> {
        let file = File::open(vol_path).unwrap();
        let dec = GzDecoder::new(file).unwrap();
        let mut arch = Archive::new(dec);
        let mut block_cont = Vec::new();
        for e in arch.entries().unwrap() {
            let mut entry = match e {
                Ok(e) => e,
                _ => {
                    break;
                }
            };
            if entry.path_bytes().as_ref() == entry_path {
                entry.read_to_end(&mut block_cont).unwrap();
                return Some(block_cont);
            }
        }
        None
    }


    #[test]
    fn snapshot() {
        let vol_path = "tests/backups/single_vol/duplicity-full.20150617T182545Z.vol1.difftar.gz";
        let snap_cache = BlockCache::new(30);
        let sig_cache = BlockCache::new(30);
        let res = TestResources {
            snap_cache: &snap_cache,
            sig_cache: &sig_cache,
            vol_path: Path::new(vol_path).to_owned(),
            vol_num: 1,
        };
        let mut stream = SnapshotStream::new(Box::new(res),
                                             RawPath::new(b"executable").as_raw_path_buf(),
                                             (0, 0),
                                             0);
        let mut buf = vec![0; BLOCK_SIZE].into_boxed_slice();
        let blen = stream.read(&mut buf[..]).unwrap();
        let block_cont = block_contents(Path::new(vol_path), b"snapshot/executable").unwrap();

        // check block result
        assert_eq!(blen, 30);
        assert!(stream.seek_to_block(1).is_err());
        assert_eq!(&block_cont[..], &buf[..blen]);

        // check cache
        assert_eq!(snap_cache.size(), 10);
        assert_eq!(sig_cache.size(), 0);
        let blen2 = snap_cache.read(((0, 0), 0), &mut buf).unwrap();
        assert_eq!(blen, blen2);
        assert_eq!(&block_cont[..], &buf[..blen]);
    }

    #[test]
    fn multivol_snapshot() {
        let vol_path = "tests/backups/single_vol/duplicity-full.20150617T182545Z.vol1.difftar.gz";
        let snap_cache = BlockCache::new(30);
        let sig_cache = BlockCache::new(30);
        let res = TestResources {
            snap_cache: &snap_cache,
            sig_cache: &sig_cache,
            vol_path: Path::new(vol_path).to_owned(),
            vol_num: 1,
        };
        let mut stream = SnapshotStream::new(Box::new(res),
                                             RawPath::new(b"largefile").as_raw_path_buf(),
                                             (0, 0),
                                             53); // the last block is 54
        let mut buf = vec![0; BLOCK_SIZE].into_boxed_slice();
        assert!(stream.seek_to_block(4).is_ok());
        let blen = stream.read(&mut buf[..]).unwrap();
        let block_cont = block_contents(Path::new(vol_path), b"multivol_snapshot/largefile/5")
            .unwrap();

        // check block result
        assert_eq!(blen, BLOCK_SIZE);
        assert_eq!(&block_cont[..], &buf[..]);

        // check cache
        assert_eq!(snap_cache.size(), 10);
        assert_eq!(sig_cache.size(), 0);
        assert!(snap_cache.cached(((0, 0), 4)));
        let blen2 = snap_cache.read(((0, 0), 4), &mut buf).unwrap();
        assert_eq!(blen, blen2);
        assert_eq!(&block_cont[..], &buf[..blen]);
    }

    #[test]
    fn path_parse_block() {
        use super::{BlockPath, BlockType, parse_block_path};

        fn block_path(e: &[u8], t: BlockType) -> BlockPath {
            BlockPath {
                entry_path: RawPath::new(e),
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
                   Some(block_path(b"foo", BlockType::MultivolSignature(4))));
        assert_eq!(parse_block_path(b"multivol_snapshot/foo/bar/5"),
                   Some(block_path(b"foo/bar", BlockType::MultivolSnapshot(4))));
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
