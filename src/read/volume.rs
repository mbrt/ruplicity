use std::borrow::Cow;
use std::io::{self, Read};
use std::str::{self, FromStr};
use std::usize;

use tar;

use read::cache::BlockCache;


pub struct VolumeReader<R: Read> {
    arch: tar::Archive<R>,
    // the id of the first path in the archive
    first_id: usize,
}


struct EntryInfo<'a> {
    path: Cow<'a, [u8]>,
    vol_num: Option<usize>,
    etype: EntryType,
}

enum EntryType {
    Deleted,
    Diff,
    Snapshot,
}

impl<R: Read> VolumeReader<R> {
    pub fn new(archive: tar::Archive<R>, first_id: usize) -> Self {
        VolumeReader {
            arch: archive,
            first_id: first_id,
        }
    }

    pub fn cache_all(&mut self, dcache: &BlockCache, ecache: &BlockCache) -> io::Result<()> {
        // io errors are treated as hard errors
        for entry in try!(self.arch.entries()) {
            let info = match EntryInfo::new(try!(entry).path_bytes()) {
                Some(info) => info,
                None => {
                    continue; // skip bad block
                }
            };
            let cache = match info.etype {
                EntryType::Deleted => {
                    continue;
                }
                EntryType::Diff => dcache,
                EntryType::Snapshot => ecache,
            };
            // TODO insert in the cache
        }
        Ok(())
    }
}


impl<'a> EntryInfo<'a> {
    pub fn new(full_path: Cow<'a, [u8]>) -> Option<Self> {
        // parse the type
        let pos = try_opt!(full_path.iter().cloned().position(|b| b == b'/'));
        let (etype, multivol) = match &full_path[0..pos] {
            b"diff" => (EntryType::Diff, false),
            b"deleted" => (EntryType::Deleted, false),
            b"snapshot" => (EntryType::Snapshot, false),
            b"multivol_diff" => (EntryType::Diff, true),
            b"multivol_snapshot" => (EntryType::Snapshot, true),
            _ => {
                return None;
            }
        };
        // parse the block number
        let epos = {
            if multivol { full_path.iter().cloned().rposition(|b| b == b'/') } else { None }
        };
        let vol_num = match epos {
            Some(pos) if pos + 1 < full_path.len() => {
                let bnum = &full_path[pos + 1..];
                str::from_utf8(bnum)
                    .ok()
                    .and_then(|strnum| usize::from_str(&strnum).ok())
            }
            _ => None,
        };
        // extract the path
        let end_pos = match epos {
            Some(epos) if epos >= pos => epos,
            _ => full_path.len(),
        };
        let path = match full_path {
            Cow::Borrowed(fp) => Cow::Borrowed(&fp[pos..end_pos]),
            Cow::Owned(fp) => Cow::Owned(fp[pos..end_pos].to_owned()),
        };

        Some(EntryInfo {
            path: path,
            vol_num: vol_num,
            etype: etype,
        })
    }
}
