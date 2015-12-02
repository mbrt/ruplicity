use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Read};
use std::iter::Iterator;
use std::path::{Component, Path, PathBuf};
use std::slice;

use flate2::read::GzDecoder;
use tar;
use time::Timespec;

use backend::Backend;
use collections::{Collections, SignatureFile};
use time_utils::to_pretty_local;


#[derive(Debug)]
pub struct BackupFiles {
    chains: Vec<Chain>,
    ug_map: UserGroupMap,
}

pub struct Snapshot<'a> {
    index: u8,
    chain: &'a Chain,
    backup: &'a BackupFiles,
}

/// Informations about a file inside a backup snapshot.
#[derive(Debug)]
pub struct File<'a> {
    path: &'a Path,
    info: &'a PathInfo,
    ug_map: &'a UserGroupMap,
}

/// A series of backup snapshots, in creation order.
pub struct Snapshots<'a> {
    chain_iter: slice::Iter<'a, Chain>,
    chain: Option<&'a Chain>,
    snapshot_id: u8,
    backup: &'a BackupFiles,
}

/// Files inside a backup snapshot.
#[derive(Clone)]
pub struct SnapshotFiles<'a> {
    index: u8,
    iter: slice::Iter<'a, PathSnapshots>,
    backup: &'a BackupFiles,
}

/// Allows to display files of a snapshot, in a `ls -s` unix command style.
pub struct SnapshotFilesDisplay<'a>(SnapshotFiles<'a>);


#[derive(Copy, Clone, Debug)]
enum DiffType {
    Signature,
    Snapshot,
    Deleted,
}

/// Store separately informations about the signatures and informations about the paths in the
/// signatures. This allows to reuse informations between snapshots and avoid duplicating them.
#[derive(Debug)]
struct Chain {
    timestamps: Vec<Timespec>,
    files: Vec<PathSnapshots>,
}

#[derive(Debug)]
struct PathSnapshots {
    // the directory or file path
    path: PathBuf,
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
    uid: Option<u32>,
    gid: Option<u32>,
    mode: Option<u32>,
    size_hint: Option<(usize, usize)>,
}

#[derive(Debug)]
struct UserGroupMap {
    uid_map: HashMap<u32, String>,
    gid_map: HashMap<u32, String>,
}

#[derive(Debug)]
struct ModeDisplay(Option<u32>);


impl BackupFiles {
    pub fn new<B: Backend>(backend: &B) -> io::Result<BackupFiles> {
        let collection = {
            let filenames = try!(backend.get_file_names());
            Collections::from_filenames(filenames)
        };
        let mut chains: Vec<Chain> = Vec::new();
        let mut ug_map = UserGroupMap::new();
        let coll_chains = collection.signature_chains();
        for coll_chain in coll_chains {
            // translate collections::SignatureChain into a Chain
            let mut chain = Chain {
                timestamps: Vec::new(),
                files: Vec::new(),
            };
            // add to the chain the full signature and all the incremental signatures
            // if an error occurs in the full signature exit
            let file = try!(backend.open_file(coll_chain.full_signature().file_name.as_ref()));
            try!(add_sigfile_to_chain(&mut chain, &mut ug_map, file, coll_chain.full_signature()));
            for inc in coll_chain.inc_signatures() {
                // TODO(#4): if an error occurs here, do not exit with an error, instead
                // break the iteration and store the error inside the chain
                let file = try!(backend.open_file(inc.file_name.as_ref()));
                try!(add_sigfile_to_chain(&mut chain, &mut ug_map, file, &inc));
            }
            chains.push(chain);
        }
        Ok(BackupFiles {
            chains: chains,
            ug_map: ug_map,
        })
    }

    pub fn snapshots(&self) -> Snapshots {
        let mut iter = self.chains.iter();
        let first_chain = iter.next();
        Snapshots {
            chain_iter: iter,
            chain: first_chain,
            snapshot_id: 0,
            backup: &self,
        }
    }
}


impl<'a> Iterator for Snapshots<'a> {
    type Item = Snapshot<'a>;

    fn next(&mut self) -> Option<Snapshot<'a>> {
        loop {
            if let Some(chain) = self.chain {
                if let Some(_) = chain.timestamps.get(self.snapshot_id as usize) {
                    let result = Some(Snapshot {
                        index: self.snapshot_id,
                        chain: chain,
                        backup: self.backup,
                    });
                    self.snapshot_id += 1;
                    return result;
                } else {
                    // this chain is completed
                    // go to next chain
                    self.chain = self.chain_iter.next();
                    self.snapshot_id = 0;
                }
            } else {
                // no other chains are present
                return None;
            }
        }
    }
}


impl<'a> Snapshot<'a> {
    pub fn time(&self) -> Timespec {
        self.chain.timestamps[self.index as usize]
    }

    pub fn files(&self) -> SnapshotFiles<'a> {
        SnapshotFiles {
            index: self.index,
            iter: self.chain.files.iter(),
            backup: self.backup,
        }
    }
}


impl<'a> Display for Snapshot<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "Backup time: {}\n{}",
               to_pretty_local(self.time()),
               self.files().to_display())
    }
}


impl<'a> SnapshotFiles<'a> {
    /// Returns a displayable struct for the files.
    ///
    /// Needs to consume `self`, because it has to iterate over all the files before displaying
    /// them, because alignment information is needed.
    pub fn to_display(self) -> SnapshotFilesDisplay<'a> {
        SnapshotFilesDisplay(self)
    }
}

impl<'a> Iterator for SnapshotFiles<'a> {
    type Item = File<'a>;

    fn next(&mut self) -> Option<File<'a>> {
        let index = self.index;     // prevents borrow checker complains
        for path_snapshots in &mut self.iter {
            if let Some(s) = path_snapshots.snapshots.iter().rev().find(|s| s.index <= index) {
                // now we have a path info present in this snapshot
                // if it is not deleted return it
                if let Some(ref info) = s.info {
                    return Some(File {
                        path: path_snapshots.path.as_ref(),
                        info: info,
                        ug_map: &self.backup.ug_map,
                    });
                }
            }
        }
        None
    }
}

impl<'a> Display for SnapshotFilesDisplay<'a> {
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


impl<'a> File<'a> {
    /// Returns the full path of the file.
    pub fn path(&self) -> &'a Path {
        self.path
    }

    pub fn userid(&self) -> Option<u32> {
        self.info.uid
    }

    pub fn groupid(&self) -> Option<u32> {
        self.info.gid
    }

    pub fn mode(&self) -> Option<u32> {
        self.info.mode
    }

    /// Returns the name of the owner user.
    pub fn username(&self) -> Option<&'a str> {
        self.info.uid.and_then(|uid| self.ug_map.get_user_name(uid))
    }

    /// Returns the name of the group.
    pub fn groupname(&self) -> Option<&'a str> {
        self.info.gid.and_then(|gid| self.ug_map.get_group_name(gid))
    }

    /// Returns the time of the last modification.
    pub fn mtime(&self) -> Timespec {
        self.info.mtime
    }

    /// Returns a lower and upper bound in bytes on the file size.
    pub fn size_hint(&self) -> Option<(usize, usize)> {
        self.info.size_hint
    }
}

impl<'a> Display for File<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f,
               "{}\t{}\t{}\t{}\t{}",
               ModeDisplay(self.mode()),
               self.username().unwrap_or("?"),
               self.groupname().unwrap_or("?"),
               to_pretty_local(self.mtime()),
               // handle special case for the root:
               // the path is empty, return "." instead
               self.path()
                   .to_str()
                   .map_or("?", |p| {
                       if p.is_empty() {
                           "."
                       } else {
                           p
                       }
                   }))
    }
}


impl UserGroupMap {
    pub fn new() -> Self {
        UserGroupMap {
            uid_map: HashMap::new(),
            gid_map: HashMap::new(),
        }
    }

    pub fn add_user(&mut self, uid: u32, name: String) -> bool {
        self.uid_map.insert(uid, name).is_none()
    }

    pub fn add_group(&mut self, gid: u32, name: String) -> bool {
        self.gid_map.insert(gid, name).is_none()
    }

    pub fn get_user_name(&self, uid: u32) -> Option<&str> {
        self.uid_map.get(&uid).map(AsRef::as_ref)
    }

    pub fn get_group_name(&self, gid: u32) -> Option<&str> {
        self.gid_map.get(&gid).map(AsRef::as_ref)
    }
}


impl Display for ModeDisplay {
    #[cfg_attr(rustfmt, rustfmt_skip)]
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
                    (true, true) => if i == 0 { "t" } else { "s" },
                    (false, true) => if i == 0 { "T" } else { "S" },
                };
                try!(write!(f, "{}{}{}", r, w, x));
            }
            Ok(())
        } else {
            write!(f, "?")
        }
    }
}


fn add_sigfile_to_chain<R: Read>(chain: &mut Chain,
                                 ug_map: &mut UserGroupMap,
                                 file: R,
                                 sigfile: &SignatureFile)
                                 -> io::Result<()> {
    let result = {
        let snapshot_id = chain.timestamps.len() as u8;
        if sigfile.compressed {
            let gz_decoder = try!(GzDecoder::new(file));
            add_sigtar_to_snapshots(&mut chain.files,
                                    ug_map,
                                    tar::Archive::new(gz_decoder),
                                    snapshot_id)
        } else {
            add_sigtar_to_snapshots(&mut chain.files,
                                    ug_map,
                                    tar::Archive::new(file),
                                    snapshot_id)
        }
    };
    if result.is_ok() {
        // add to the list of signatures only if everything is ok
        // we do not need to cleanup the chain if someting went wrong, because if the
        // list of signatures is not updated, the change is not observable
        chain.timestamps.push(sigfile.time);
    }
    result
}

fn add_sigtar_to_snapshots<R: Read>(snapshots: &mut Vec<PathSnapshots>,
                                    ug_map: &mut UserGroupMap,
                                    mut tar: tar::Archive<R>,
                                    snapshot_id: u8)
                                    -> io::Result<()> {
    let mut new_files: Vec<PathSnapshots> = Vec::new();
    {
        let mut old_snapshots = snapshots.iter_mut().peekable();
        for tarfile in try!(tar.files_mut()) {
            // we can ignore paths with errors
            // the only problem here is that we miss some change in the chain, but it is
            // better than abort the whole signature
            let mut tarfile = unwrap_or_continue!(tarfile);
            let size_hint = compute_size_hint(&mut tarfile);
            let path = unwrap_or_continue!(tarfile.header().path());
            let (difftype, path) = unwrap_opt_or_continue!(parse_snapshot_path(&path));
            let info = match difftype {
                DiffType::Signature | DiffType::Snapshot => {
                    let header = tarfile.header();
                    let time = Timespec::new(header.mtime().unwrap_or(0) as i64, 0);
                    if let (Ok(uid), Some(name)) = (header.uid(), header.username()) {
                        ug_map.add_user(uid, name.to_owned());
                    }
                    if let (Ok(gid), Some(name)) = (header.gid(), header.groupname()) {
                        ug_map.add_group(gid, name.to_owned());
                    }
                    Some(PathInfo {
                        mtime: time,
                        uid: header.uid().ok(),
                        gid: header.gid().ok(),
                        mode: header.mode().ok(),
                        size_hint: size_hint,
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
                        let old_path = path_snapshots.path.as_path();
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
                    } else {
                        // we have not found the element, so 'old_path < path' or there are no
                        // more paths to check:
                        // continue the loop if there are more elements
                        if !old_snapshots.next().is_some() {
                            break;
                        }
                    }
                }
                position
            };
            if let Some(path_snapshots) = position {
                path_snapshots.snapshots.push(new_snapshot);
            } else {
                // the path is not present in the old snapshots: add to new list
                new_files.push(PathSnapshots {
                    path: path.to_path_buf(),
                    snapshots: vec![new_snapshot],
                });
            }
        }
    }
    // merge the new files with old snapshots
    if !new_files.is_empty() {
        // TODO: Performance hurt here: we have two sorted arrays to merge,
        // better to use this algorithm: http://stackoverflow.com/a/4553321/1667955
        snapshots.extend(new_files.into_iter());
        snapshots.sort_by(|a, b| a.path.cmp(&b.path));
    }
    Ok(())
}

fn parse_snapshot_path(path: &Path) -> Option<(DiffType, &Path)> {
    // split the path in (first directory, the remaining path)
    // the first is the type, the remaining is the real path
    let mut pcomps = path.components();
    let pfirst = try_opt!(pcomps.next());
    if let Component::Normal(strfirst) = pfirst {
        let difftype = match strfirst.to_str() {
            Some("signature") => DiffType::Signature,
            Some("snapshot") => DiffType::Snapshot,
            Some("deleted") => DiffType::Deleted,
            _ => {
                return None;
            }
        };
        let realpath = pcomps.as_path();
        Some((difftype, realpath))
    } else {
        None
    }
}

fn compute_size_hint<R: Read>(file: &mut tar::File<R>) -> Option<(usize, usize)> {
    let difftype = {
        let path = try_opt!(file.header().path().ok());
        let (difftype, _) = try_opt!(parse_snapshot_path(&path));
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
///
/// # Examples
///
/// ```rust
/// use std::io::Cursor;
/// use ruplicity::signatures::compute_size_hint_signature;
///
/// let bytes = vec![0x72, 0x73, 0x01, 0x36, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x08,
///                  0xaf, 0xb8, 0x99, 0x27, 0x6f, 0x3a, 0x17, 0xc2, 0xc1, 0x4e, 0x76, 0x83];
/// let mut cursor = Cursor::new(bytes);
/// let computed = compute_size_hint_signature(&mut cursor);
/// assert_eq!(computed, Some((0, 512)));
/// ```
pub fn compute_size_hint_signature<R: Read>(file: &mut R) -> Option<(usize, usize)> {
    use byteorder::{BigEndian, ReadBytesExt};

    // for signature file format see Docs.md
    let magic = try_opt!(file.read_u32::<BigEndian>().ok());
    if magic != 0x72730136 {
        None
    } else {
        // read the header
        let file_block_len_bytes = try_opt!(file.read_u32::<BigEndian>().ok()) as usize;
        let ss_len = try_opt!(file.read_u32::<BigEndian>().ok()) as usize;
        let sign_block_len_bytes = 4 + ss_len;
        // the remaining part of the file are blocks
        let num_blocks = file.bytes().count() / sign_block_len_bytes;

        let max_file_len = file_block_len_bytes * num_blocks;
        if max_file_len > file_block_len_bytes {
            Some((max_file_len - file_block_len_bytes + 1, max_file_len))
        } else {
            // avoid underflow
            Some((0, max_file_len))
        }
    }
}

fn compute_size_hint_snapshot<R: Read>(file: &mut R) -> Option<(usize, usize)> {
    let bytes = file.bytes().count();
    Some((bytes, bytes))
}

// used for tests only
#[cfg(test)]
#[doc(hidden)]
pub fn _mode_display(mode: Option<u32>) -> String {
    format!("{}", ModeDisplay(mode))
}


#[cfg(test)]
mod test {
    use super::*;
    use backend::local::LocalBackend;
    use time_utils::{parse_time_str, to_pretty_local};
    use time_utils::test_utils::set_time_zone;

    use std::path::Path;
    use time::Timespec;


    #[derive(Debug, Clone, Eq, PartialEq)]
    struct FileTest<'a> {
        path: &'a Path,
        mtime: Timespec,
        uname: &'a str,
        gname: &'a str,
    }

    impl<'a> FileTest<'a> {
        pub fn from_file(file: &File<'a>) -> Self {
            FileTest {
                path: file.path(),
                mtime: file.mtime(),
                uname: file.username().unwrap(),
                gname: file.groupname().unwrap(),
            }
        }

        pub fn from_info(path: &'a Path, mtime: &'a str, uname: &'a str, gname: &'a str) -> Self {
            FileTest {
                path: path,
                mtime: parse_time_str(mtime).unwrap(),
                uname: uname,
                gname: gname,
            }
        }
    }

    fn make_ftest<'a>(path: &'a str, time: &'a str) -> FileTest<'a> {
        FileTest::from_info(Path::new(path), time, "michele", "michele")
    }

    fn get_single_vol_files() -> Vec<Vec<FileTest<'static>>> {
        // the utf-8 invalid path name is apparently not testable
        // so, we are going to ignore it
        //
        // snapshot 1
        let s1 = vec![make_ftest("", "20020928t183059z"),
                      make_ftest("changeable_permission", "20010828t182330z"),
                      make_ftest("deleted_file", "20020727t230005z"),
                      make_ftest("directory_to_file", "20020727t230036z"),
                      make_ftest("directory_to_file/file", "20020727t230036z"),
                      make_ftest("executable", "20010828t073429z"),
                      make_ftest("executable2", "20010828t181927z"),
                      make_ftest("fifo", "20010828t073246z"),
                      make_ftest("file_to_directory", "20020727t232354z"),
                      make_ftest("largefile", "20020731t015430z"),
                      make_ftest("regular_file", "20010828t073052z"),
                      make_ftest("regular_file.sig", "20010830t004037z"),
                      make_ftest("symbolic_link", "20021101t044447z"),
                      make_ftest("test", "20010828t215638z"),
                      make_ftest("two_hardlinked_files1", "20010828t073142z"),
                      make_ftest("two_hardlinked_files2", "20010828t073142z")];
        // snapshot 2
        let s2 = vec![make_ftest("", "20020731t015532z"),
                      make_ftest("changeable_permission", "20010828t182330z"),
                      make_ftest("directory_to_file", "20020727t230048z"),
                      make_ftest("executable", "20010828t073429z"),
                      make_ftest("executable2", "20020727t230133z"),
                      make_ftest("executable2/another_file", "20020727t230133z"),
                      make_ftest("fifo", "20010828t073246z"),
                      make_ftest("file_to_directory", "20020727t232406z"),
                      make_ftest("largefile", "20020731t015524z"),
                      make_ftest("new_file", "20020727t230018z"),
                      make_ftest("regular_file", "20020727t225932z"),
                      make_ftest("regular_file.sig", "20010830t004037z"),
                      make_ftest("symbolic_link", "20020727t225946z"),
                      make_ftest("test", "20010828t215638z"),
                      make_ftest("two_hardlinked_files1", "20010828t073142z"),
                      make_ftest("two_hardlinked_files2", "20010828t073142z")];
        // snapshot 3
        let s3 = vec![make_ftest("", "20020928t183059z"),
                      make_ftest("changeable_permission", "20010828t182330z"),
                      make_ftest("executable", "20010828t073429z"),
                      make_ftest("executable2", "20010828t181927z"),
                      make_ftest("fifo", "20010828t073246z"),
                      make_ftest("largefile", "20020731t034334z"),
                      make_ftest("regular_file", "20010828t073052z"),
                      make_ftest("regular_file.sig", "20010830t004037z"),
                      make_ftest("symbolic_link", "20021101t044448z"),
                      make_ftest("test", "20010828t215638z"),
                      make_ftest("two_hardlinked_files1", "20010828t073142z"),
                      make_ftest("two_hardlinked_files2", "20010828t073142z")];

        vec![s1, s2, s3]
    }

    fn get_single_vol_sizes() -> Vec<Vec<usize>> {
        // note that `ls -l` returns 4096 for directory size, but we consider directories to be
        // null sized.
        // note also that symbolic links are considered to be null sized. This is an open question
        // if it is correct or not.
        vec![vec![0, 0, 0, 0, 0, 30, 30, 0, 456, 3500000, 75650, 456, 0, 0, 11, 11, 0],
             vec![0, 0, 456, 30, 0, 13, 0, 0, 3500001, 6, 75656, 456, 0, 0, 11, 11, 0],
             vec![0, 0, 30, 30, 0, 3500000, 75650, 456, 0, 0, 11, 11, 0]]
    }

    #[test]
    fn file_list() {
        let expected_files = get_single_vol_files();
        let backend = LocalBackend::new("tests/backups/single_vol");
        let files = BackupFiles::new(&backend).unwrap();
        // println!("debug files\n---------\n{:#?}\n----------", files);
        let actual_files = files.snapshots().map(|s| {
            s.files()
             .map(|f| FileTest::from_file(&f))
             .filter(|f| f.path.to_str().is_some())
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
        let backend = LocalBackend::new("tests/backups/single_vol");
        let files = BackupFiles::new(&backend).unwrap();
        let actual_sizes = files.snapshots().map(|s| {
            s.files()
             .map(|f| f.size_hint().unwrap())
             .collect::<Vec<_>>()
        });
        let expected_sizes = get_single_vol_sizes();

        // iterate all over the snapshots
        for (actual, expected) in actual_sizes.zip(expected_sizes) {
            assert_eq!(actual.len(), expected.len());
            // println!("debug {:?}", actual);
            // iterate all the files
            for (actual, expected) in actual.iter().zip(expected) {
                assert!(actual.0 <= expected && actual.1 >= expected,
                        "failed: valid interval: [{} - {}], real value: {}",
                        actual.0,
                        actual.1,
                        expected);
            }
        }
    }

    #[test]
    fn display() {
        // avoid test differences for time zones
        let _lock = set_time_zone("Europe/Rome");

        let backend = LocalBackend::new("tests/backups/single_vol");
        let files = BackupFiles::new(&backend).unwrap();
        println!("Backup snapshots:");
        for snapshot in files.snapshots() {
            println!("Snapshot {}\n{}",
                     to_pretty_local(snapshot.time()),
                     snapshot.files().to_display());
        }
    }

    #[test]
    fn mode_display() {
        // see http://permissions-calculator.org/symbolic/
        // for help on permissions
        assert_eq!(_mode_display(None), "?");
        assert_eq!(_mode_display(Some(0o777)), "rwxrwxrwx");
        assert_eq!(_mode_display(Some(0o000)), "---------");
        assert_eq!(_mode_display(Some(0o444)), "r--r--r--");
        assert_eq!(_mode_display(Some(0o700)), "rwx------");
        assert_eq!(_mode_display(Some(0o542)), "r-xr---w-");
        assert_eq!(_mode_display(Some(0o4100)), "--s------");
        assert_eq!(_mode_display(Some(0o4000)), "--S------");
        assert_eq!(_mode_display(Some(0o7000)), "--S--S--T");
        assert_eq!(_mode_display(Some(0o7111)), "--s--s--t");
    }
}
