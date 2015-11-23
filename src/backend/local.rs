use super::Backend;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::ffi::OsString;


/// Backend operating on the local filesystem.
pub struct LocalBackend {
    base_path: PathBuf,
}

/// Iterator over a set of file names.
pub struct FileNameIterator(fs::ReadDir);


impl LocalBackend {
    /// Create a new LocalBackend that operates on the given directory.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        LocalBackend { base_path: path.as_ref().to_path_buf() }
    }
}

impl Backend for LocalBackend {
    type FileName = OsString;
    type FileNameIter = FileNameIterator;
    type FileStream = File;

    fn get_file_names(&self) -> io::Result<Self::FileNameIter> {
        let dir = try!(fs::read_dir(self.base_path.as_path()));
        Ok(FileNameIterator(dir))
    }

    fn open_file(&self, name: &Path) -> io::Result<File> {
        let mut path = self.base_path.clone();
        path.push(name);
        File::open(path)
    }
}

impl Iterator for FileNameIterator {
    type Item = OsString;

    fn next(&mut self) -> Option<OsString> {
        for entry in &mut self.0 {
            if let Ok(entry) = entry {
                return Some(entry.file_name());
            }
        }
        None
    }
}
