use super::Backend;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};


/// Backend operating on the local filesystem.
pub struct LocalBackend {
    base_path: PathBuf,
}

impl LocalBackend {
    /// Create a new LocalBackend that operates on the given directory.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        LocalBackend { base_path: path.as_ref().to_path_buf() }
    }
}

impl Backend for LocalBackend {
    type FileName = PathBuf;
    type FileNameIter = Vec<PathBuf>;
    type FileStream = File;

    fn get_file_names(&self) -> io::Result<Self::FileNameIter> {
        let dir = try!(fs::read_dir(self.base_path.as_path()));
        let paths = dir.filter(|entry| entry.is_ok())
                       .map(|entry| {
                           let filename = entry.unwrap().file_name();
                           let filename: &Path = filename.as_ref();
                           filename.to_path_buf()
                       })
                       .collect::<Vec<_>>();
        Ok(paths)
    }

    fn open_file(&self, name: &Path) -> io::Result<File> {
        let mut path = self.base_path.clone();
        path.push(name);
        File::open(path)
    }
}
