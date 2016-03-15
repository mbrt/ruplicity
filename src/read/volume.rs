use std::borrow::Cow;
use std::io::Read;
use std::str::{self, FromStr};
use std::usize;

use tar;

pub type VolumeReader<R> = tar::Archive<R>;
pub type VolumeReaderIter<'a, R> = tar::Entries<'a, R>;

pub struct VolumeEntry<'a, R: Read + 'a>(tar::Entry<'a, R>);

pub struct EntryInfo<'a> {
    path: Cow<'a, [u8]>,
    vol_num: Option<usize>,
    etype: EntryType,
}

pub enum EntryType {
    Deleted,
    Diff,
    Snapshot,
}


impl<'a, R: Read + 'a> VolumeEntry<'a, R> {
    pub fn info(&self) -> Option<EntryInfo> {
        EntryInfo::new(self.0.path_bytes())
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
            Cow::Borrowed(full_path) => Cow::Borrowed(&full_path[pos..end_pos]),
            Cow::Owned(full_path) => Cow::Owned(full_path[pos..end_pos].to_owned()),
        };

        Some(EntryInfo {
            path: path,
            vol_num: vol_num,
            etype: etype,
        })
    }
}
