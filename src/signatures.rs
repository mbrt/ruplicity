use std::io;
use std::iter::Iterator;
use std::slice;

use flate2::read::GzDecoder;
use tar;
use time::Timespec;

use collections::{CollectionsStatus, SignatureChain, SignatureFile};
use backend::Backend;


pub struct BackupFiles {
    snapshots: Vec<Snapshot>
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


impl BackupFiles {
    pub fn new<B: Backend>(backend: &B) -> io::Result<BackupFiles> {
        let collection = {
            let filenames = try!(backend.get_file_names());
            CollectionsStatus::from_filenames(&filenames)
        };
        let chains = collection.signature_chains();
        for chain in chains {
            let file = try!(backend.open_file(chain.fullsig.file_name.as_ref()));
            if chain.fullsig.compressed {
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
                        let ptail = pcomps.as_path();
                    }
                }
            }
        }
        // TODO: go from signature chains to snapshots
        Ok(BackupFiles{ snapshots: Vec::new() })
    }

    pub fn snapshots(&self) -> Snapshots {
        self.snapshots.iter()
    }
}

