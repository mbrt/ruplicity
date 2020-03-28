//! Operations on backup signatures.
//!
//! This sub-module exposes types to deal with duplicity signatures. It can be used to get
//! information about files backupped in a backup chain.

use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Read};
use std::iter::Iterator;
use std::path::Path;
use std::slice;

use flate2::read::GzDecoder;
use tar;
use time::Timespec;

use crate::backend::Backend;
use crate::collections::{SignatureChain, SignatureFile};
use crate::rawpath::RawPath;
use crate::timefmt::TimeDisplay;

/// Stores information about paths in a backup chain.
///
/// The information is reused among different snapshots if possible.
#[derive(Debug)]
pub struct Chain {
    num_snapshots: u8,
    files: Vec<PathSnapshots>,
    ug_map: UserGroupMap,
}

/// Signatures for backup snapshots, in creation order.
#[derive(Debug)]
pub struct Snapshots<'a> {
    chain: &'a Chain,
    snapshot_id: u8,
}

/// A signature for a backup snapshot.
#[derive(Debug)]
pub struct Snapshot<'a> {
    chain: &'a Chain,
    index: u8,
}

/// Files and directories inside a backup snapshot.
#[derive(Clone)]
pub struct SnapshotEntries<'a> {
    index: u8,
    iter: slice::Iter<'a, PathSnapshots>,
    chain: &'a Chain,
}

/// Allows to display files of a snapshot.
///
/// The style used is similar to the one used by `ls -l` unix command.
pub struct SnapshotEntriesDisplay<'a>(SnapshotEntries<'a>);

/// Information about an entry inside a backup snapshot.
///
/// This could be a file, a directory, a link, etc.
#[derive(Debug)]
pub struct Entry<'a> {
    path: &'a RawPath,
    info: &'a PathInfo,
    ug_map: &'a UserGroupMap,
}

/// Type of entry in a backup snapshot.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EntryType {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// An hard link.
    ///
    /// This entry type is currently not supported by duplicity.
    HardLink,
    /// A symbolic link.
    SymLink,
    /// An unix pipe.
    Fifo,
    /// All the other entry types, that are currently not managed.
    Unknown(u8),
}

#[derive(Copy, Clone, Debug)]
enum DiffType {
    Signature,
    Snapshot,
    Deleted,
}

#[derive(Debug)]
struct PathSnapshots {
    // the directory or file path
    path: RawPath,
    // all the snapshots for this path
    snapshots: Vec<PathSnapshot>,
}

#[derive(Debug)]
struct PathSnapshot {
    // info are None if the snapshot has deleted this path
    info: Option<PathInfo>,
    // the index of the snapshot in the chain
    index: u8,
}

#[derive(Debug)]
struct PathInfo {
    mtime: Timespec,
    uid: Option<u64>,
    gid: Option<u64>,
    mode: Option<u32>,
    entry_type: u8,
    size_hint: Option<(usize, usize)>,
    link: Option<RawPath>,
}

#[derive(Debug)]
struct UserGroupMap {
    uid_map: HashMap<u64, String>,
    gid_map: HashMap<u64, String>,
}

#[derive(Debug)]
struct ModeDisplay(Option<u32>);

impl Chain {
    /// Builds a new empty signature chain.
    pub fn new() -> Self {
        Chain {
            num_snapshots: 0,
            files: Vec::new(),
            ug_map: UserGroupMap::new(),
        }
    }

    /// Opens a signature chain from signature chain files, by using a backend.
    ///
    /// The given signature chain file names are read by using the given backend, to build the
    /// corresponding `Chain` instance.
    pub fn from_sigchain<B: Backend>(coll: &SignatureChain, backend: &B) -> io::Result<Self> {
        let mut chain = Chain::new();
        // add to the chain the full signature and all the incremental signatures
        // if an error occurs in the full signature exit
        let file = backend.open_file(coll.full_signature().file_name.as_ref())?;
        chain.add_sigfile(file, coll.full_signature())?;
        for inc in coll.inc_signatures() {
            // TODO(#4): if an error occurs here, do not exit with an error, instead
            // break the iteration and store the error inside the chain
            let file = backend.open_file(inc.file_name.as_ref())?;
            chain.add_sigfile(file, &inc)?;
        }
        Ok(chain)
    }

    /// Returns the snapshots present in the signature chain.
    pub fn snapshots(&self) -> Snapshots {
        Snapshots {
            chain: self,
            snapshot_id: 0,
        }
    }

    fn add_sigfile<R: Read>(&mut self, file: R, sigfile: &SignatureFile) -> io::Result<()> {
        let result = {
            let snapshot_id = self.num_snapshots;
            if sigfile.compressed {
                let gz_decoder = GzDecoder::new(file)?;
                self.add_sigtar_to_snapshots(tar::Archive::new(gz_decoder), snapshot_id)
            } else {
                self.add_sigtar_to_snapshots(tar::Archive::new(file), snapshot_id)
            }
        };
        if result.is_ok() {
            // add to the list of snapshots only if everything is ok
            // we do not need to cleanup the chain if someting went wrong, because if the
            // number of signatures is not updated, the change is not observable
            self.num_snapshots += 1;
        }
        result
    }

    fn add_sigtar_to_snapshots<R: Read>(
        &mut self,
        mut tar: tar::Archive<R>,
        snapshot_id: u8,
    ) -> io::Result<()> {
        let mut new_files: Vec<PathSnapshots> = Vec::new();
        {
            let mut old_snapshots = self.files.iter_mut().peekable();
            for tarfile in tar.entries()? {
                // we can ignore paths with errors
                // the only problem here is that we miss some change in the chain, but it is
                // better than abort the whole signature
                let mut tarfile = unwrap_or_continue!(tarfile);
                let size_hint = compute_size_hint(&mut tarfile);
                let path = &tarfile.path_bytes();
                let (difftype, path) = unwrap_opt_or_continue!(parse_snapshot_path(&path));
                let info = match difftype {
                    DiffType::Signature | DiffType::Snapshot => {
                        let header = tarfile.header();
                        let time = Timespec::new(header.mtime().unwrap_or(0) as i64, 0);
                        if let (Ok(uid), Ok(Some(name))) = (header.uid(), header.username()) {
                            self.ug_map.add_user(uid, name.to_owned());
                        }
                        if let (Ok(gid), Ok(Some(name))) = (header.gid(), header.groupname()) {
                            self.ug_map.add_group(gid, name.to_owned());
                        }
                        let link = tarfile
                            .link_name_bytes()
                            .map(|b| RawPath::from_bytes(b.into_owned()));
                        Some(PathInfo {
                            mtime: time,
                            uid: header.uid().ok(),
                            gid: header.gid().ok(),
                            mode: header.mode().ok(),
                            size_hint: size_hint,
                            entry_type: tarfile.header().entry_type().as_byte(),
                            link: link,
                        })
                    }
                    _ => None,
                };
                let new_snapshot = PathSnapshot {
                    info: info,
                    index: snapshot_id,
                };
                // find the current path in the old snapshots
                // note: they are ordered
                let position = {
                    let mut position: Option<&mut PathSnapshots> = None;
                    loop {
                        let mut found = false;
                        if let Some(path_snapshots) = old_snapshots.peek() {
                            let old_path = path_snapshots.path.as_bytes();
                            if old_path == path {
                                // this path is already present in old snapshots: update them
                                found = true;
                            } else if old_path > path {
                                // we've already reached the first item next to the current path
                                // so, the path is not present in old snapshots
                                break;
                            }
                        }
                        if found {
                            let path_snapshots = old_snapshots.next().unwrap();
                            position = Some(path_snapshots);
                        } else if !old_snapshots.next().is_some() {
                            // we have not found the element, so 'old_path < path' or there are no
                            // more paths to check:
                            // continue the loop if there are more elements
                            break;
                        }
                    }
                    position
                };
                if let Some(path_snapshots) = position {
                    path_snapshots.snapshots.push(new_snapshot);
                } else {
                    // the path is not present in the old snapshots: add to new list
                    new_files.push(PathSnapshots {
                        path: RawPath::from_bytes(path.to_owned()),
                        snapshots: vec![new_snapshot],
                    });
                }
            }
        }
        // merge the new files with old snapshots
        if !new_files.is_empty() {
            // TODO: Performance hurt here: we have two sorted arrays to merge,
            // better to use this algorithm: http://stackoverflow.com/a/4553321/1667955
            self.files.extend(new_files.into_iter());
            self.files.sort_by(|a, b| a.path.cmp(&b.path));
        }
        Ok(())
    }
}

// some optimizations are implemented for snapshots iteration, like `nth` and `ExactSizeIterator`.
impl<'a> Iterator for Snapshots<'a> {
    type Item = Snapshot<'a>;

    fn next(&mut self) -> Option<Snapshot<'a>> {
        if self.snapshot_id < self.chain.num_snapshots {
            self.snapshot_id += 1;
            Some(Snapshot {
                chain: self.chain,
                index: self.snapshot_id - 1,
            })
        } else {
            None
        }
    }

    fn nth(&mut self, n: usize) -> Option<Snapshot<'a>> {
        use std::u8;

        // check for u8 overflow to be fool-proof
        if n + self.snapshot_id as usize >= u8::MAX as usize {
            return None;
        }
        let id = self.snapshot_id + n as u8;
        if id < self.chain.num_snapshots {
            self.snapshot_id = id + 1;
            Some(Snapshot {
                chain: self.chain,
                index: id,
            })
        } else {
            None
        }
    }
}

impl<'a> ExactSizeIterator for Snapshots<'a> {
    fn len(&self) -> usize {
        (self.chain.num_snapshots - self.snapshot_id) as usize
    }
}

impl<'a> Snapshot<'a> {
    /// Returns the files inside this backup snapshot.
    pub fn files(&self) -> SnapshotEntries<'a> {
        SnapshotEntries {
            index: self.index,
            iter: self.chain.files.iter(),
            chain: self.chain,
        }
    }
}

impl<'a> Display for Snapshot<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.files().into_display())
    }
}

impl<'a> SnapshotEntries<'a> {
    /// Returns a displayable struct for the files.
    ///
    /// Needs to consume `self`, because it has to iterate over all the files to align the output
    /// columns properly.
    pub fn into_display(self) -> SnapshotEntriesDisplay<'a> {
        SnapshotEntriesDisplay(self)
    }
}

impl<'a> Iterator for SnapshotEntries<'a> {
    type Item = Entry<'a>;

    fn next(&mut self) -> Option<Entry<'a>> {
        let index = self.index; // prevents borrow checker complains
        for path_snapshots in &mut self.iter {
            if let Some(s) = path_snapshots
                .snapshots
                .iter()
                .rev()
                .find(|s| s.index <= index)
            {
                // now we have a path info present in this snapshot
                // if it is not deleted return it
                if let Some(ref info) = s.info {
                    return Some(Entry {
                        path: &path_snapshots.path,
                        info: info,
                        ug_map: &self.chain.ug_map,
                    });
                }
            }
        }
        None
    }
}

impl<'a> Display for SnapshotEntriesDisplay<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        use std::io::Write;
        use tabwriter::TabWriter;

        let mut tw = TabWriter::new(Vec::new());
        for file in self.0.clone() {
            try_or_fmt_err!(write!(&mut tw, "{}\n", file));
        }
        try_or_fmt_err!(tw.flush());
        let written = try_or_fmt_err!(String::from_utf8(tw.unwrap()));
        write!(f, "{}", written)
    }
}

impl<'a> Entry<'a> {
    /// Returns the full path of the entry.
    ///
    /// The path could be `None` if it is not UTF-8 representable under Windows. In that case use
    /// the byte representation with `path_bytes`.
    pub fn path(&self) -> Option<&'a Path> {
        self.path.as_path()
    }

    /// Returns the full path of the entry in bytes.
    pub fn path_bytes(&self) -> &'a [u8] {
        self.path.as_bytes()
    }

    /// Returns the value of the owner's user ID field.
    pub fn userid(&self) -> Option<u64> {
        self.info.uid
    }

    /// Returns the value of the group's user ID field.
    pub fn groupid(&self) -> Option<u64> {
        self.info.gid
    }

    /// Returns the mode bits for this file.
    pub fn mode(&self) -> Option<u32> {
        self.info.mode
    }

    /// Returns the name of the owner user.
    pub fn username(&self) -> Option<&'a str> {
        self.info.uid.and_then(|uid| self.ug_map.get_user_name(uid))
    }

    /// Returns the name of the group.
    pub fn groupname(&self) -> Option<&'a str> {
        self.info
            .gid
            .and_then(|gid| self.ug_map.get_group_name(gid))
    }

    /// Returns the last modification time.
    pub fn mtime(&self) -> Timespec {
        self.info.mtime
    }

    /// Returns a lower and upper bound in bytes on the entry size.
    ///
    /// Note that for directories, this returns a size of zero, even if on Linux directories are
    /// often considered to have a 4096 bytes size.
    pub fn size_hint(&self) -> Option<(usize, usize)> {
        self.info.size_hint
    }

    /// Returns the type of the entry.
    pub fn entry_type(&self) -> EntryType {
        EntryType::new(self.info.entry_type)
    }

    /// Returns the path that this entry points to.
    ///
    /// This will return some path only if this entry is a symbolic link.
    pub fn linked_path(&self) -> Option<&'a Path> {
        self.info.link.as_ref().and_then(|p| p.as_path())
    }
}

impl<'a> Display for Entry<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}{}\t{}\t{}\t{}\t{}\t{}",
            self.entry_type(),
            ModeDisplay(self.mode()),
            self.username().unwrap_or("?"),
            self.groupname().unwrap_or("?"),
            self.size_hint()
                .map_or("?".to_owned(), |hint| format!("{}", hint.1)),
            self.mtime().into_local_display(),
            // handle special case for the root:
            // the path is empty, return "." instead
            self.path
        )
    }
}

impl EntryType {
    /// Creates a new entry type from a raw byte.
    ///
    /// The enumeration is taken from TAR file specification.
    pub fn new(byte: u8) -> EntryType {
        match byte {
            0 | b'0' => EntryType::File,
            b'5' => EntryType::Dir,
            b'1' => EntryType::HardLink,
            b'2' => EntryType::SymLink,
            b'6' => EntryType::Fifo,
            _ => EntryType::Unknown(byte),
        }
    }
}

impl Display for EntryType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            match *self {
                EntryType::File => '-',
                EntryType::Dir => 'd',
                EntryType::HardLink => '-',
                EntryType::SymLink => 'l',
                EntryType::Fifo => 'p',
                EntryType::Unknown(_) => '?',
            }
        )
    }
}

impl UserGroupMap {
    pub fn new() -> Self {
        UserGroupMap {
            uid_map: HashMap::new(),
            gid_map: HashMap::new(),
        }
    }

    pub fn add_user(&mut self, uid: u64, name: String) -> bool {
        self.uid_map.insert(uid, name).is_none()
    }

    pub fn add_group(&mut self, gid: u64, name: String) -> bool {
        self.gid_map.insert(gid, name).is_none()
    }

    pub fn get_user_name(&self, uid: u64) -> Option<&str> {
        self.uid_map.get(&uid).map(AsRef::as_ref)
    }

    pub fn get_group_name(&self, gid: u64) -> Option<&str> {
        self.gid_map.get(&gid).map(AsRef::as_ref)
    }
}

impl Display for ModeDisplay {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        // from octal permissions to rwx ls style
        if let Some(mode) = self.0 {
            let special = mode >> 9;
            // index iterates over user, group, other
            for i in (0..3).rev() {
                let curr = mode >> (i * 3);
                let r = if curr & 0b100 > 0 { "r" } else { "-" };
                let w = if curr & 0b010 > 0 { "w" } else { "-" };
                // executable must handle the special permissions
                let x = match (curr & 0b001 > 0, special & (1 << i) > 0) {
                    (true, false) => "x",
                    (false, false) => "-",
                    (true, true) => {
                        if i == 0 {
                            "t"
                        } else {
                            "s"
                        }
                    }
                    (false, true) => {
                        if i == 0 {
                            "T"
                        } else {
                            "S"
                        }
                    }
                };
                write!(f, "{}{}{}", r, w, x)?;
            }
            Ok(())
        } else {
            write!(f, "?")
        }
    }
}

fn parse_snapshot_path(path: &[u8]) -> Option<(DiffType, &[u8])> {
    // split the path in (first directory, the remaining path)
    // the first is the type, the remaining is the real path
    let pos = path.iter().cloned().position(|b| b == b'/')?;
    let (pfirst, raw_real) = path.split_at(pos);
    let difftype = match pfirst {
        b"signature" => DiffType::Signature,
        b"snapshot" => DiffType::Snapshot,
        b"deleted" => DiffType::Deleted,
        _ => {
            return None;
        }
    };
    let real = match raw_real.last().cloned() {
        Some(b'/') if raw_real.len() > 1 => &raw_real[1..raw_real.len() - 1],
        _ => &raw_real[1..],
    };
    Some((difftype, real))
}

fn compute_size_hint<R: Read>(file: &mut tar::Entry<R>) -> Option<(usize, usize)> {
    let difftype = {
        let path = &file.path_bytes();
        let (difftype, _) = parse_snapshot_path(&path)?;
        difftype
    };
    match difftype {
        DiffType::Signature => compute_size_hint_signature(file),
        DiffType::Snapshot => compute_size_hint_snapshot(file),
        _ => None,
    }
}

/// Gives a hint on the file size, computing it from the signature file.
///
/// This function returns the lower and upper bound of the file size in bytes. On error returns
/// `None`.
fn compute_size_hint_signature<R: Read>(file: &mut tar::Entry<R>) -> Option<(usize, usize)> {
    use byteorder::{BigEndian, ReadBytesExt};

    // for signature file format see Docs.md
    let magic = file.read_u32::<BigEndian>().ok()?;
    if magic != 0x72730136 {
        None
    } else {
        // read the header
        let file_block_len_bytes = file.read_u32::<BigEndian>().ok()? as usize;
        let ss_len = file.read_u32::<BigEndian>().ok()? as usize;
        let sign_block_len_bytes = 4 + ss_len;
        // the remaining part of the file are blocks
        let file_size = file.header().size().ok()? as usize;
        let num_blocks = (file_size - 8) / sign_block_len_bytes;

        let max_file_len = file_block_len_bytes * num_blocks;
        if max_file_len > file_block_len_bytes {
            Some((max_file_len - file_block_len_bytes + 1, max_file_len))
        } else {
            // avoid underflow
            Some((0, max_file_len))
        }
    }
}

fn compute_size_hint_snapshot<R: Read>(file: &mut tar::Entry<R>) -> Option<(usize, usize)> {
    let bytes = file.header().size().ok()? as usize;
    Some((bytes, bytes))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::backend::local::LocalBackend;
    use crate::backend::Backend;
    use crate::collections::Collections;
    use crate::timefmt::parse_time_str;

    use std::path::{Path, PathBuf};
    use time::Timespec;

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct EntryTest {
        path: Vec<u8>,
        mtime: Timespec,
        uname: String,
        gname: String,
        entry_type: EntryType,
        link: Option<PathBuf>,
    }

    impl EntryTest {
        pub fn from_entry(file: &Entry) -> Self {
            EntryTest {
                path: file.path_bytes().to_owned(),
                mtime: file.mtime(),
                uname: file.username().unwrap().to_owned(),
                gname: file.groupname().unwrap().to_owned(),
                entry_type: file.entry_type(),
                link: file.linked_path().map(ToOwned::to_owned),
            }
        }

        pub fn from_info(
            path: &[u8],
            mtime: &str,
            uname: &str,
            gname: &str,
            etype: EntryType,
            link: Option<&Path>,
        ) -> Self {
            EntryTest {
                path: path.to_owned(),
                mtime: parse_time_str(mtime).unwrap(),
                uname: uname.to_owned(),
                gname: gname.to_owned(),
                entry_type: etype,
                link: link.map(ToOwned::to_owned),
            }
        }
    }

    fn make_ftest(path: &[u8], time: &str, etype: EntryType) -> EntryTest {
        EntryTest::from_info(path, time, "michele", "michele", etype, None)
    }

    fn make_ftest_link(path: &[u8], time: &str, link: &str) -> EntryTest {
        EntryTest::from_info(
            path,
            time,
            "michele",
            "michele",
            EntryType::SymLink,
            Some(Path::new(link)),
        )
    }

    fn single_vol_expected_files() -> Vec<Vec<EntryTest>> {
        let nonutf8path = vec![
            0xd8, 0xab, 0xb1, 0x57, 0x62, 0xae, 0xc5, 0x5d, 0x8a, 0xbb, 0x15, 0x76, 0x2a, 0xf4,
            0x0f, 0x21, 0xf9, 0x3e, 0xe2, 0x59, 0x86, 0xbb, 0xab, 0xdb, 0x70, 0xb0, 0x84, 0x13,
            0x6b, 0x1d, 0xc2, 0xf1, 0xf5, 0x65, 0xa5, 0x55, 0x82, 0x9a, 0x55, 0x56, 0xa0, 0xf4,
            0xdf, 0x34, 0xba, 0xfd, 0x58, 0x03, 0x82, 0x07, 0x73, 0xce, 0x9e, 0x8b, 0xb3, 0x34,
            0x04, 0x9f, 0x17, 0x20, 0xf4, 0x8f, 0xa6, 0xfa, 0x97, 0xab, 0xd8, 0xac, 0xda, 0x85,
            0xdc, 0x4b, 0x76, 0x43, 0xfa, 0x23, 0x94, 0x92, 0x9e, 0xc9, 0xb7, 0xc3, 0x5f, 0x0f,
            0x84, 0x67, 0x9a, 0x42, 0x11, 0x3c, 0x3d, 0x5e, 0xdb, 0x4d, 0x13, 0x96, 0x63, 0x8b,
            0xa7, 0x7c, 0x2a, 0x22, 0x5c, 0x27, 0x5e, 0x24, 0x40, 0x23, 0x21, 0x28, 0x29, 0x7b,
            0x7d, 0x3f, 0x2b, 0x20, 0x7e, 0x60, 0x20,
        ];
        // snapshot 1
        let s1 = vec![
            make_ftest(b"", "20020928t183059z", EntryType::Dir),
            make_ftest(
                b"changeable_permission",
                "20010828t182330z",
                EntryType::File,
            ),
            make_ftest(b"deleted_file", "20020727t230005z", EntryType::File),
            make_ftest(b"directory_to_file", "20020727t230036z", EntryType::Dir),
            make_ftest(
                b"directory_to_file/file",
                "20020727t230036z",
                EntryType::File,
            ),
            make_ftest(b"executable", "20010828t073429z", EntryType::File),
            make_ftest(b"executable2", "20010828t181927z", EntryType::File),
            make_ftest(b"fifo", "20010828t073246z", EntryType::Fifo),
            make_ftest(b"file_to_directory", "20020727t232354z", EntryType::File),
            make_ftest(b"largefile", "20020731t015430z", EntryType::File),
            make_ftest(b"regular_file", "20010828t073052z", EntryType::File),
            make_ftest(b"regular_file.sig", "20010830t004037z", EntryType::File),
            make_ftest_link(b"symbolic_link", "20021101t044447z", "regular_file"),
            make_ftest(b"test", "20010828t215638z", EntryType::File),
            make_ftest(
                b"two_hardlinked_files1",
                "20010828t073142z",
                EntryType::File,
            ),
            make_ftest(
                b"two_hardlinked_files2",
                "20010828t073142z",
                EntryType::File,
            ),
            make_ftest(&nonutf8path, "20010828t220347z", EntryType::File),
        ];
        // snapshot 2
        let s2 = vec![
            make_ftest(b"", "20020731t015532z", EntryType::Dir),
            make_ftest(
                b"changeable_permission",
                "20010828t182330z",
                EntryType::File,
            ),
            make_ftest(b"directory_to_file", "20020727t230048z", EntryType::File),
            make_ftest(b"executable", "20010828t073429z", EntryType::File),
            make_ftest(b"executable2", "20020727t230133z", EntryType::Dir),
            make_ftest(
                b"executable2/another_file",
                "20020727t230133z",
                EntryType::File,
            ),
            make_ftest(b"fifo", "20010828t073246z", EntryType::Fifo),
            make_ftest(b"file_to_directory", "20020727t232406z", EntryType::Dir),
            make_ftest(b"largefile", "20020731t015524z", EntryType::File),
            make_ftest(b"new_file", "20020727t230018z", EntryType::File),
            make_ftest(b"regular_file", "20020727t225932z", EntryType::File),
            make_ftest(b"regular_file.sig", "20010830t004037z", EntryType::File),
            make_ftest(b"symbolic_link", "20020727t225946z", EntryType::Dir),
            make_ftest(b"test", "20010828t215638z", EntryType::File),
            make_ftest(
                b"two_hardlinked_files1",
                "20010828t073142z",
                EntryType::File,
            ),
            make_ftest(
                b"two_hardlinked_files2",
                "20010828t073142z",
                EntryType::File,
            ),
            make_ftest(&nonutf8path, "20010828t220347z", EntryType::File),
        ];
        // snapshot 3
        let s3 = vec![
            make_ftest(b"", "20020928t183059z", EntryType::Dir),
            make_ftest(
                b"changeable_permission",
                "20010828t182330z",
                EntryType::File,
            ),
            make_ftest(b"executable", "20010828t073429z", EntryType::File),
            make_ftest(b"executable2", "20010828t181927z", EntryType::File),
            make_ftest(b"fifo", "20010828t073246z", EntryType::Fifo),
            make_ftest(b"largefile", "20020731t034334z", EntryType::File),
            make_ftest(b"regular_file", "20010828t073052z", EntryType::File),
            make_ftest(b"regular_file.sig", "20010830t004037z", EntryType::File),
            make_ftest_link(b"symbolic_link", "20021101t044448z", "regular_file"),
            make_ftest(b"test", "20010828t215638z", EntryType::File),
            make_ftest(
                b"two_hardlinked_files1",
                "20010828t073142z",
                EntryType::File,
            ),
            make_ftest(
                b"two_hardlinked_files2",
                "20010828t073142z",
                EntryType::File,
            ),
            make_ftest(&nonutf8path, "20010828t220347z", EntryType::File),
        ];

        vec![s1, s2, s3]
    }

    fn single_vol_files() -> Chain {
        let backend = LocalBackend::new("tests/backups/single_vol");
        let filenames = backend.file_names().unwrap();
        let coll = Collections::from_filenames(filenames);
        Chain::from_sigchain(coll.signature_chains().next().unwrap(), &backend).unwrap()
    }

    fn single_vol_sizes() -> Vec<Vec<usize>> {
        // note that `ls -l` returns 4096 for directory size, but we consider directories to be
        // null sized.
        // note also that symbolic links are considered to be null sized. This is an open question
        // if it is correct or not.
        vec![
            vec![
                0, 0, 0, 0, 0, 30, 30, 0, 456, 3500000, 75650, 456, 0, 0, 11, 11, 0,
            ],
            vec![
                0, 0, 456, 30, 0, 13, 0, 0, 3500001, 6, 75656, 456, 0, 0, 11, 11, 0,
            ],
            vec![0, 0, 30, 30, 0, 3500000, 75650, 456, 0, 0, 11, 11, 0],
        ]
    }

    #[test]
    fn file_list() {
        let expected_files = single_vol_expected_files();
        let files = single_vol_files();
        // println!("debug files\n---------\n{:#?}\n----------", files);
        let actual_files = files.snapshots().map(|s| {
            s.files()
                .map(|f| EntryTest::from_entry(&f))
                .collect::<Vec<_>>()
        });
        assert_eq!(files.snapshots().count(), 3);
        for (actual, expected) in actual_files.zip(expected_files) {
            // println!("\nExpected:\n{:#?}\nActual:\n{:#?}", expected, actual);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn size_hint() {
        let files = single_vol_files();
        let actual_sizes = files.snapshots().map(|s| {
            s.files()
                .map(|f| f.size_hint().unwrap())
                .collect::<Vec<_>>()
        });
        let expected_sizes = single_vol_sizes();

        // iterate all over the snapshots
        for (actual, expected) in actual_sizes.zip(expected_sizes) {
            // println!("debug {:?}", actual);
            assert_eq!(actual.len(), expected.len());
            // iterate all the files
            for (actual, expected) in actual.iter().zip(expected) {
                assert!(
                    actual.0 <= expected && actual.1 >= expected,
                    "failed: valid interval: [{} - {}], real value: {}",
                    actual.0,
                    actual.1,
                    expected
                );
            }
        }
    }

    #[test]
    fn display() {
        // NOTE: this is actually not a proper test
        //       here we are only printing out the snapshots.
        //       however not panicking is already something :)
        //       Display is not properly testable due to time zones differencies;
        //       we want to avoid using global mutexes in test code
        let files = single_vol_files();
        println!("Backup snapshots:\n");
        for snapshot in files.snapshots() {
            println!("Snapshot\n{}", snapshot.files().into_display());
        }
    }

    #[test]
    fn mode_display() {
        fn mode_display(mode: Option<u32>) -> String {
            use super::ModeDisplay;
            format!("{}", ModeDisplay(mode))
        }

        // see http://permissions-calculator.org/symbolic/
        // for help on permissions
        assert_eq!(mode_display(None), "?");
        assert_eq!(mode_display(Some(0o777)), "rwxrwxrwx");
        assert_eq!(mode_display(Some(0o000)), "---------");
        assert_eq!(mode_display(Some(0o444)), "r--r--r--");
        assert_eq!(mode_display(Some(0o700)), "rwx------");
        assert_eq!(mode_display(Some(0o542)), "r-xr---w-");
        assert_eq!(mode_display(Some(0o4100)), "--s------");
        assert_eq!(mode_display(Some(0o4000)), "--S------");
        assert_eq!(mode_display(Some(0o7000)), "--S--S--T");
        assert_eq!(mode_display(Some(0o7111)), "--s--s--t");
    }
}
