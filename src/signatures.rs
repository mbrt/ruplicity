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

impl BackupFiles {
    pub fn new<B: Backend>(backend: &B) -> io::Result<BackupFiles> {
        let builder = BackupFilesBuilder{ backend: backend };
        builder.build()
    }

    pub fn snapshots(&self) -> Snapshots {
        self.snapshots.iter()
    }

//    fn signatures_files(chain: &SignatureChain) -> Vec<TarHeaderIter> {
//        unimplemented!()
//    }

//    fn signature_file_iter(&self, signature: &SignatureFile) -> io::Result<BoxTarHeaderIter> {
//        let file = try!(self.backend.open_file(signature.file_name.as_ref()));
//        if signature.compressed {
//            let gz_decoder = try!(GzDecoder::new(file));
//            let mut tar = tar::Archive::new(gz_decoder);
//        }
//        unimplemented!()
//    }
}


struct BackupFilesBuilder<'a, B: Backend + 'a> {
    backend: &'a B
}

impl<'a, B: Backend> BackupFilesBuilder<'a, B> {
    pub fn build(&self) -> io::Result<BackupFiles> {
        let collection = {
            let filenames = try!(self.backend.get_file_names());
            CollectionsStatus::from_filenames(&filenames)
        };
        let chains = collection.signature_chains();
        // TODO: go from signature chains to snapshots
        Ok(BackupFiles{ snapshots: Vec::new() })
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
