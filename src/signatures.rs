use std::io;
use std::iter::Iterator;
use std::slice;

use flate2::read::GzDecoder;
use tar;
use time::Timespec;

use collections::{CollectionsStatus, SignatureChain, SignatureFile};
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

trait TarArchive {
    fn file_headers(&mut self) -> io::Result<Box<TarHeaderIter>>;
}

impl<R: io::Read> TarArchive for tar::Archive<R> {
    fn file_headers(&mut self) -> io::Result<Box<TarHeaderIter>> {
        let files = try!(self.files_mut());
        Ok(Box::new(TarHeaderIterImpl(files)))
    }
}


type TarHeaderIter<'a> = Iterator<Item=&'a tar::Header>;

struct TarHeaderIterImpl<'a, R: 'a>(tar::FilesMut<'a, R>);

impl<'a, R> Iterator for TarHeaderIterImpl<'a, R> {
    type Item = &'a tar::Header;

    fn next(&mut self) -> Option<Self::Item> {
        None
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
