use std::io::{self, Read};
use std::iter::Iterator;
use std::slice;
use std::path::{Component, PathBuf};

use flate2::read::GzDecoder;
use tar;
use time::Timespec;

use backend::Backend;
use collections::{CollectionsStatus, SignatureChain, SignatureFile};


pub struct BackupFiles {
    chains: Vec<Chain>
}

pub struct Snapshot {
    pub time: Timespec
}

pub struct File {
    pub name: String,
    pub last_modified: Timespec
}

/// Iterator over a list of backup snapshots.
pub type Snapshots<'a> = slice::Iter<'a, Snapshot>;


enum DiffType {
    Signature,
    Snapshot,
    Deleted
}

/// Store separately informations about the signatures and informations about the paths in the
/// signatures. This allows to reuse informations between snapshots and avoid duplicating them.
struct Chain {
    timestamps: Vec<Timespec>,
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
    mtime: Timespec
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
            let mut chain = Chain{ timestamps: Vec::new(), files: Vec::new() };
            let file = try!(backend.open_file(coll_chain.fullsig.file_name.as_ref()));
            add_sigfile_to_chain(&mut chain, file, &coll_chain.fullsig);
            for inc in coll_chain.inclist.iter() {
                let file = try!(backend.open_file(inc.file_name.as_ref()));
                add_sigfile_to_chain(&mut chain, file, &inc);
            }
            chains.push(chain);
        }
        Ok(BackupFiles{ chains: chains })
    }

    pub fn snapshots(&self) -> Snapshots {
        unimplemented!()
    }
}


fn add_sigfile_to_chain<R: Read>(chain: &mut Chain, file: R, sigfile: &SignatureFile) -> io::Result<()> {
    if sigfile.compressed {
        let gz_decoder = try!(GzDecoder::new(file));
        let mut tar = tar::Archive::new(gz_decoder);
        for tarfile in try!(tar.files_mut()) {
            if let Ok(tarfile) = tarfile {
                let header = tarfile.header();
                let path = unwrap_or_continue!(header.path());
                let mut pcomps = path.components();
                // split the path in (first directory, the remaining path)
                // the first is the type, the remaining is the real path
                let pfirst = unwrap_opt_or_continue!(pcomps.next());
                if let Component::Normal(strfirst) = pfirst {
                    let difftype = match strfirst.to_str() {
                        Some("signature") => DiffType::Signature,
                        Some("snapshot")  => DiffType::Snapshot,
                        Some("deleted")   => DiffType::Deleted,
                        _                 => { continue; }
                    };
                    let realpath = pcomps.as_path();
                }
            }
        }
    }
    Ok(())
}

