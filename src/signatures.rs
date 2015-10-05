use std::convert::From;
use std::io::{self, Read};
use std::iter::Iterator;
use std::path::{Component, Path, PathBuf};
use std::slice;

use flate2::read::GzDecoder;
use tar;
use time::Timespec;

use backend::Backend;
use collections::{CollectionsStatus, SignatureFile};


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
            // translate collections::SignatureChain into a Chain
            let mut chain = Chain{ timestamps: Vec::new(), files: Vec::new() };
            // add to the chain the full signature and all the incremental signatures
            // if an error occurs in the full signature exit
            let file = try!(backend.open_file(coll_chain.fullsig.file_name.as_ref()));
            try!(add_sigfile_to_chain(&mut chain, file, &coll_chain.fullsig));
            for inc in coll_chain.inclist.iter() {
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
        unimplemented!()
    }
}


fn add_sigfile_to_chain<R: Read>(chain: &mut Chain, file: R, sigfile: &SignatureFile) -> io::Result<()> {
    let result = {
        if sigfile.compressed {
            let gz_decoder = try!(GzDecoder::new(file));
            add_sigtar_to_chain(chain, tar::Archive::new(gz_decoder))
        }
        else {
            add_sigtar_to_chain(chain, tar::Archive::new(file))
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

fn add_sigtar_to_chain<R: Read>(chain: &mut Chain, mut tar: tar::Archive<R>) -> io::Result<()> {
    for tarfile in try!(tar.files_mut()) {
        // we can ignore paths with errors
        // the only problem here is that we miss some change in the chain, but it is better
        // than abort the whole signature
        let tarfile = unwrap_or_continue!(tarfile);
        let header = tarfile.header();
        let path = unwrap_or_continue!(header.path());
        let (difftype, path) = unwrap_opt_or_continue!(parse_snapshot_path(&path));
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

