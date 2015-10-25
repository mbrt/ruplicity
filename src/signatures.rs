use std::io::{self, Read};
use std::iter::Iterator;
use std::path::{Component, Path, PathBuf};
use std::slice;

use chrono::DateTime;
use flate2::read::GzDecoder;
use tar;

use backend::Backend;
use collections::{CollectionsStatus, SignatureFile};
use time_utils::Timestamp;


pub struct BackupFiles {
    chains: Vec<Chain>
}

pub struct Snapshot<'a> {
    index: u8,
    chain: &'a Chain
}

#[derive(Debug)]
pub struct File<'a> {
    pub path: &'a Path,
    pub last_modified: Timestamp
}

/// Iterator over a list of backup snapshots.
pub struct Snapshots<'a> {
    chain_iter: slice::Iter<'a, Chain>,
    chain: Option<&'a Chain>,
    snapshot_id: u8
}

pub struct SnapshotFiles<'a> {
    index: u8,
    iter: slice::Iter<'a, PathSnapshots>
}


enum DiffType {
    Signature,
    Snapshot,
    Deleted
}

/// Store separately informations about the signatures and informations about the paths in the
/// signatures. This allows to reuse informations between snapshots and avoid duplicating them.
struct Chain {
    timestamps: Vec<Timestamp>,
    files: Vec<PathSnapshots>
}

struct PathSnapshots {
    // the directory or file path
    path: PathBuf,
    // all the snapshots for this path
    snapshots: Vec<PathSnapshot>
}

struct PathSnapshot {
    // info are None if the snapshot has deleted this path
    info: Option<PathInfo>,
    // the index of the snapshot in the chain
    index: u8
}

/// Informations about a path inside a snapshot.
struct PathInfo {
    mtime: Timestamp
}


impl BackupFiles {
    pub fn new<B: Backend>(backend: &B) -> io::Result<BackupFiles> {
        let collection = {
            let filenames = try!(backend.get_file_names());
            CollectionsStatus::from_filenames(&filenames)
        };
        let mut chains: Vec<Chain> = Vec::new();
        let coll_chains = collection.signature_chains();
        for coll_chain in coll_chains {
            // translate collections::SignatureChain into a Chain
            let mut chain = Chain{ timestamps: Vec::new(), files: Vec::new() };
            // add to the chain the full signature and all the incremental signatures
            // if an error occurs in the full signature exit
            let file = try!(backend.open_file(coll_chain.fullsig.file_name.as_ref()));
            try!(add_sigfile_to_chain(&mut chain, file, &coll_chain.fullsig));
            for inc in &coll_chain.inclist {
                // TODO: if an error occurs here, do not exit with an error, instead
                // break the iteration and store the error inside the chain
                let file = try!(backend.open_file(inc.file_name.as_ref()));
                try!(add_sigfile_to_chain(&mut chain, file, &inc));
            }
            chains.push(chain);
        }
        Ok(BackupFiles{ chains: chains })
    }

    pub fn snapshots(&self) -> Snapshots {
        let mut iter = self.chains.iter();
        let first_chain = iter.next();
        Snapshots{ chain_iter: iter, chain: first_chain, snapshot_id: 0 }
    }
}


impl<'a> Iterator for Snapshots<'a> {
    type Item = Snapshot<'a>;

    fn next(&mut self) -> Option<Snapshot<'a>> {
        loop {
            if let Some(chain) = self.chain {
                if let Some(_) = chain.timestamps.get(self.snapshot_id as usize) {
                    let result = Some(Snapshot{ index: self.snapshot_id, chain: chain });
                    self.snapshot_id += 1;
                    return result;
                }
                else {
                    // this chain is completed
                    // go to next chain
                    self.chain = self.chain_iter.next();
                    self.snapshot_id = 0;
                }
            }
            else {
                // no other chains are present
                return None
            }
        }
    }
}


impl<'a> Snapshot<'a> {
    pub fn time(&self) -> Timestamp {
        self.chain.timestamps[self.index as usize]
    }

    pub fn files(&self) -> SnapshotFiles<'a> {
        SnapshotFiles{ index: self.index, iter: self.chain.files.iter() }
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
                    return Some(File{
                        path: path_snapshots.path.as_ref(),
                        last_modified: info.mtime
                    });
                }
            }
        }
        None
    }
}


fn add_sigfile_to_chain<R: Read>(chain: &mut Chain,
                                 file: R,
                                 sigfile: &SignatureFile) -> io::Result<()>
{
    let result = {
        let snapshot_id = chain.files.len() as u8;
        if sigfile.compressed {
            let gz_decoder = try!(GzDecoder::new(file));
            add_sigtar_to_snapshots(&mut chain.files, tar::Archive::new(gz_decoder), snapshot_id)
        }
        else {
            add_sigtar_to_snapshots(&mut chain.files, tar::Archive::new(file), snapshot_id)
        }
    };
    if result.is_ok() {
        // add to the list of signatures only if everything is ok
        // we do not need to cleanup the chain if someting went wrong, because if the list of
        // signatures is not updated, the change is not observable
        chain.timestamps.push(sigfile.time);
    }
    result
}

fn add_sigtar_to_snapshots<R: Read>(snapshots: &mut Vec<PathSnapshots>,
                                    mut tar: tar::Archive<R>,
                                    snapshot_id: u8) -> io::Result<()>
{
    let mut new_files: Vec<PathSnapshots> = Vec::new();
    {
        let mut old_snapshots = snapshots.iter_mut();
        for tarfile in try!(tar.files_mut()) {
            // we can ignore paths with errors
            // the only problem here is that we miss some change in the chain, but it is better
            // than abort the whole signature
            let tarfile = unwrap_or_continue!(tarfile);
            let header = tarfile.header();
            let path = unwrap_or_continue!(header.path());
            let (difftype, path) = unwrap_opt_or_continue!(parse_snapshot_path(&path));
            let info = match difftype {
                DiffType::Signature | DiffType::Snapshot => {
                    let time = header.mtime().unwrap_or(0 as u64) as i64;
                    Some(PathInfo{ mtime: time })
                }
                _ => None
            };
            let new_snapshot = PathSnapshot{ info: info, index: snapshot_id };
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
                    }
                    else {
                        // we've already reached the first item next to the current path
                        // so, the path is not present in old snapshots
                        position = None;
                    }
                    break;
                };
                position
            };
            if let Some(path_snapshots) = position {
                path_snapshots.snapshots.push(new_snapshot);
            }
            else {
                // the path is not present in the old snapshots: add to new list
                new_files.push(PathSnapshots{
                    path: path.to_path_buf(),
                    snapshots: vec![new_snapshot]
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
            Some("snapshot")  => DiffType::Snapshot,
            Some("deleted")   => DiffType::Deleted,
            _                 => { return None; }
        };
        let realpath = pcomps.as_path();
        Some((difftype, realpath))
    }
    else {
        None
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use backend::local::LocalBackend;
    use time_utils::to_pretty_local;


    #[test]
    fn open_from_local() {
        let backend = LocalBackend::new("tests/backups/single_vol").unwrap();
        let files = BackupFiles::new(&backend).unwrap();
        println!("Backup snapshots:");
        for snapshot in files.snapshots() {
            println!("Snapshot {}", to_pretty_local(snapshot.time()));
            for file in snapshot.files() {
                println!("    {:?}", file);
            }
        }
    }
}
