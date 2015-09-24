use std::slice;
use std::path::Path;
use std::fs;
use std::io;
use time::Timespec;

use collections::CollectionsStatus;


pub struct BackupFiles {
    snapshots : Vec<Snapshot>
}

impl BackupFiles {
    pub fn from_dir<P : AsRef<Path>>(path : P) -> io::Result<BackupFiles> {
        let filenames = try!(Self::collect_filenames(path));
        let collection = CollectionsStatus::from_filename_list(&filenames);
        // TODO: go from signature chains to snapshots
        Ok(BackupFiles{ snapshots : Vec::new() })
    }

    pub fn snapshots(&self) -> Snapshots {
        self.snapshots.iter()
    }

    fn collect_filenames<P : AsRef<Path>>(path : P) -> io::Result<Vec<String>> {
        let paths = try!(fs::read_dir(path));
        let mut filenames : Vec<String> = Vec::new();
        for entry in paths {
            let entry = unwrap_or_continue!(entry);
            let filename = unwrap_or_continue!(entry.file_name().into_string());
            filenames.push(filename);
        }
        Ok(filenames)
    }
}

/// Iterator over a list of backup snapshots.
pub type Snapshots<'a> = slice::Iter<'a, Snapshot>;


pub struct Snapshot {
    pub time : Timespec
}

// impl Snapshot {
//     pub fn files(&self) ->
// }

pub struct File {
    pub name : String,
    pub last_modified : Timespec
}
