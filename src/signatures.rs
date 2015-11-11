use std::collections::HashMap;
use std::fmt::{Display, Error, Formatter};
use std::io::{self, Read};
use std::iter::Iterator;
use std::path::{Component, Path, PathBuf};
use std::slice;

use flate2::read::GzDecoder;
use tar;
use time::Timespec;

use backend::Backend;
use collections::{CollectionsStatus, SignatureFile};
use time_utils::to_pretty_local;


pub struct BackupFiles {
    chains: Vec<Chain>,
    ug_cache: UserGroupNameCache,
}

pub struct Snapshot<'a> {
    index: u8,
    chain: &'a Chain,
    ug_cache: &'a UserGroupNameCache,
}

/// Informations about a file inside a backup snapshot.
#[derive(Debug)]
pub struct File<'a> {
    path: &'a Path,
    info: &'a PathInfo,
    ug_cache: &'a UserGroupNameCache,
}

/// Iterator over a list of backup snapshots.
pub struct Snapshots<'a> {
    chain_iter: slice::Iter<'a, Chain>,
    chain: Option<&'a Chain>,
    snapshot_id: u8,
    ug_cache: &'a UserGroupNameCache,
}

pub struct SnapshotFiles<'a> {
    index: u8,
    iter: slice::Iter<'a, PathSnapshots>,
    ug_cache: &'a UserGroupNameCache,
}


enum DiffType {
    Signature,
    Snapshot,
    Deleted,
}

/// Store separately informations about the signatures and informations about the paths in the
/// signatures. This allows to reuse informations between snapshots and avoid duplicating them.
struct Chain {
    timestamps: Vec<Timespec>,
    files: Vec<PathSnapshots>,
}

struct PathSnapshots {
    // the directory or file path
    path: PathBuf,
    // all the snapshots for this path
    snapshots: Vec<PathSnapshot>,
}

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
}

#[derive(Debug)]
struct UserGroupNameCache {
    uid_map: HashMap<u32, String>,
    gid_map: HashMap<u32, String>,
}


impl BackupFiles {
    pub fn new<B: Backend>(backend: &B) -> io::Result<BackupFiles> {
        let collection = {
            let filenames = try!(backend.get_file_names());
            CollectionsStatus::from_filenames(&filenames)
        };
        let mut chains: Vec<Chain> = Vec::new();
        let mut ug_cache = UserGroupNameCache::new();
        let coll_chains = collection.signature_chains();
        for coll_chain in coll_chains {
            // translate collections::SignatureChain into a Chain
            let mut chain = Chain {
                timestamps: Vec::new(),
                files: Vec::new(),
            };
            // add to the chain the full signature and all the incremental signatures
            // if an error occurs in the full signature exit
            let file = try!(backend.open_file(coll_chain.fullsig.file_name.as_ref()));
            try!(add_sigfile_to_chain(&mut chain, &mut ug_cache, file, &coll_chain.fullsig));
            for inc in &coll_chain.inclist {
                // TODO: if an error occurs here, do not exit with an error, instead
                // break the iteration and store the error inside the chain
                let file = try!(backend.open_file(inc.file_name.as_ref()));
                try!(add_sigfile_to_chain(&mut chain, &mut ug_cache, file, &inc));
            }
            chains.push(chain);
        }
        Ok(BackupFiles {
            chains: chains,
            ug_cache: ug_cache,
        })
    }

    pub fn snapshots(&self) -> Snapshots {
        let mut iter = self.chains.iter();
        let first_chain = iter.next();
        Snapshots {
            chain_iter: iter,
            chain: first_chain,
            snapshot_id: 0,
            ug_cache: &self.ug_cache,
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
                        ug_cache: self.ug_cache,
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
            ug_cache: self.ug_cache,
        }
    }
}


impl<'a> Iterator for SnapshotFiles<'a> {
    type Item = File<'a>;

    fn next(&mut self) -> Option<File<'a>> {
        while let Some(path_snapshots) = self.iter.next() {
            if let Some(s) = path_snapshots.snapshots.iter().rev().find(|s| s.index <= self.index) {
                // now we have a path info present in this snapshot
                // if it is not deleted return it
                if let Some(ref info) = s.info {
                    return Some(File {
                        path: path_snapshots.path.as_ref(),
                        info: info,
                        ug_cache: self.ug_cache,
                    });
                }
            }
        }
        None
    }
}


impl<'a> File<'a> {
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

    pub fn username(&self) -> Option<&'a str> {
        self.info.uid.and_then(|uid| self.ug_cache.get_user_name(uid))
    }

    pub fn groupname(&self) -> Option<&'a str> {
        self.info.gid.and_then(|gid| self.ug_cache.get_group_name(gid))
    }

    pub fn mtime(&self) -> Timespec {
        self.info.mtime
    }
}

impl<'a> Display for File<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f,
               "{:<4} {:<10} {:<10} {} {}",
               self.mode().unwrap_or(0),
               self.username().unwrap_or("?"),
               self.groupname().unwrap_or("?"),
               // FIXME: Workaround for rust <= 1.4
               // Alignment is ignored by custom formatters
               // see: https://github.com/rust-lang-deprecated/time/issues/98#issuecomment-103010106
               format!("{}", to_pretty_local(self.mtime())),
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


impl UserGroupNameCache {
    pub fn new() -> Self {
        UserGroupNameCache {
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


fn add_sigfile_to_chain<R: Read>(chain: &mut Chain,
                                 ug_cache: &mut UserGroupNameCache,
                                 file: R,
                                 sigfile: &SignatureFile)
                                 -> io::Result<()> {
    let result = {
        let snapshot_id = chain.files.len() as u8;
        if sigfile.compressed {
            let gz_decoder = try!(GzDecoder::new(file));
            add_sigtar_to_snapshots(&mut chain.files,
                                    ug_cache,
                                    tar::Archive::new(gz_decoder),
                                    snapshot_id)
        } else {
            add_sigtar_to_snapshots(&mut chain.files,
                                    ug_cache,
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
                                    ug_cache: &mut UserGroupNameCache,
                                    mut tar: tar::Archive<R>,
                                    snapshot_id: u8)
                                    -> io::Result<()> {
    let mut new_files: Vec<PathSnapshots> = Vec::new();
    {
        let mut old_snapshots = snapshots.iter_mut();
        for tarfile in try!(tar.files_mut()) {
            // we can ignore paths with errors
            // the only problem here is that we miss some change in the chain, but it is
            // better than abort the whole signature
            let tarfile = unwrap_or_continue!(tarfile);
            let header = tarfile.header();
            let path = unwrap_or_continue!(header.path());
            let (difftype, path) = unwrap_opt_or_continue!(parse_snapshot_path(&path));
            let info = match difftype {
                DiffType::Signature | DiffType::Snapshot => {
                    let time = Timespec::new(header.mtime().unwrap_or(0) as i64, 0);
                    if let (Ok(uid), Some(name)) = (header.uid(), header.username()) {
                        ug_cache.add_user(uid, name.to_owned());
                    }
                    if let (Ok(gid), Some(name)) = (header.gid(), header.groupname()) {
                        ug_cache.add_group(gid, name.to_owned());
                    }
                    Some(PathInfo {
                        mtime: time,
                        uid: header.uid().ok(),
                        gid: header.gid().ok(),
                        mode: header.mode().ok(),
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
                while let Some(path_snapshots) = old_snapshots.next() {
                    if path_snapshots.path.as_path() < path {
                        continue;
                    }
                    if path_snapshots.path.as_path() == path {
                        // this path is already present in old snapshots: update them
                        position = Some(path_snapshots);
                    } else {
                        // we've already reached the first item next to the current path
                        // so, the path is not present in old snapshots
                        position = None;
                    }
                    break;
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

    fn get_single_vol_files() -> Vec<Vec<FileTest<'static>>> {
        // the utf-8 invalid path name is apparently not testable
        // so, we are going to ignore it
        //
        // snapshot 1
        let s1 = vec![FileTest::from_info(Path::new(""), "20020928t183059z", "michele", "michele"),
                      FileTest::from_info(Path::new("changeable_permission"),
                                          "20010828t182330z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("deleted_file"),
                                          "20020727t230005z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("directory_to_file"),
                                          "20020727t230036z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("directory_to_file/file"),
                                          "20020727t230036z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("executable"),
                                          "20010828t073429z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("executable2"),
                                          "20010828t181927z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("fifo"),
                                          "20010828t073246z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("file_to_directory"),
                                          "20020727t232354z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("largefile"),
                                          "20020731t015430z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("regular_file"),
                                          "20010828t073052z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("regular_file.sig"),
                                          "20010830t004037z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("symbolic_link"),
                                          "20021101t044447z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("test"),
                                          "20010828t215638z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("two_hardlinked_files1"),
                                          "20010828t073142z",
                                          "michele",
                                          "michele"),
                      FileTest::from_info(Path::new("two_hardlinked_files2"),
                                          "20010828t073142z",
                                          "michele",
                                          "michele")];
        // snapshot 2
        let mut s2 = s1.clone();
        // .
        s2[0].mtime = parse_time_str("20020731t015532z").unwrap();
        s2.remove(2);   // deleted_file
        s2.remove(3);   // directory_to_file/file
        // executable2
        s2[4].mtime = parse_time_str("20020731t230133z").unwrap();
        s2.insert(5,
                  FileTest::from_info(Path::new("executable2/another_file"),
                                      "20020727t230033z",
                                      "michele",
                                      "michele"));
        s2.insert(9,
                  FileTest::from_info(Path::new("new_file"),
                                      "20020727t230018z",
                                      "michele",
                                      "michele"));
        // symbolic_link
        s2[12].mtime = parse_time_str("20020727t225946z").unwrap();

        vec![s1, s2]
    }

    #[test]
    fn file_list() {
        let expected_files = get_single_vol_files();
        let backend = LocalBackend::new("tests/backups/single_vol").unwrap();
        let files = BackupFiles::new(&backend).unwrap();
        assert_eq!(files.snapshots().count(), 3);
        let actual_files = files.snapshots().map(|s| {
            let f: Vec<_> = s.files()
                             .map(|f| FileTest::from_file(&f))
                             .filter(|f| f.path.to_str().is_some())
                             .collect();
            f
        });
        for (actual, expected) in actual_files.zip(expected_files) {
            assert_eq!(actual, expected);
        }
    }


    #[test]
    fn display() {
        // avoid test differences for time zones
        let _lock = set_time_zone("Europe/Rome");

        let backend = LocalBackend::new("tests/backups/single_vol").unwrap();
        let files = BackupFiles::new(&backend).unwrap();
        println!("Backup snapshots:");
        for snapshot in files.snapshots() {
            println!("Snapshot {}", to_pretty_local(snapshot.time()));
            for file in snapshot.files() {
                println!("{}", file);
            }
        }
    }
}
