use std::slice;
use std::io;
use time::Timespec;

use collections::CollectionsStatus;
use backend::Backend;


pub struct BackupFiles<B> {
    backend: B,
    snapshots: Vec<Snapshot>
}

impl<B: Backend> BackupFiles<B> {
    pub fn new(backend: B) -> io::Result<BackupFiles<B>> {
        let collection = {
            let filenames = try!(backend.get_file_names());
            CollectionsStatus::from_filenames(&filenames)
        };
        let chains = collection.signature_chains();
        // TODO: go from signature chains to snapshots
        Ok(BackupFiles{ backend: backend, snapshots: Vec::new() })
    }

    pub fn snapshots(&self) -> Snapshots {
        self.snapshots.iter()
    }
}

/// Iterator over a list of backup snapshots.
pub type Snapshots<'a> = slice::Iter<'a, Snapshot>;


pub struct Snapshot {
    pub time: Timespec
}

// impl Snapshot {
//     pub fn files(&self) ->
// }

pub struct File {
    pub name: String,
    pub last_modified: Timespec
}
