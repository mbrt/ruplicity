use std::io::{self, Read};
use std::path::Path;


/// Backend is a trait used to provide access to backup files.
pub trait Backend {
    /// FileName is an associated type for a file name. It must be convertible to a string
    /// reference.
    type FileName: AsRef<Path>;

    /// FileStream is an associated type for a read stream for a file.
    type FileStream: Read;

    /// Returns a list of available file names.
    /// The file names returned should have an extension, and not a path.
    fn get_file_names(&self) -> io::Result<&[Self::FileName]>;

    /// Open a file for reading.
    fn open_file(&self, name: &str) -> io::Result<Self::FileStream>;
}


pub mod local {
    use super::Backend;
    use std::fs::{self, File};
    use std::io;
    use std::path::{Path, PathBuf};


    /// Backend operating on the local filesystem.
    pub struct LocalBackend {
        base_path: PathBuf,
        file_names: Vec<PathBuf>
    }

    impl LocalBackend {
        /// Create a new LocalBackend that operates on the given directory.
        pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
            let paths = try!(fs::read_dir(path.as_ref()));
            let mut filenames = Vec::new();
            for entry in paths {
                let entry = unwrap_or_continue!(entry);
                let filename = entry.file_name();
                let filename: &Path = filename.as_ref();
                filenames.push(filename.to_path_buf());
            }
            Ok(LocalBackend{
                base_path: path.as_ref().to_path_buf(),
                file_names: filenames
            })
        }
    }

    impl Backend for LocalBackend {
        type FileName = PathBuf;
        type FileStream = File;

        fn get_file_names(&self) -> io::Result<&[PathBuf]> {
            Ok(&self.file_names)
        }

        fn open_file(&self, name: &str) -> io::Result<File> {
            let mut path = self.base_path.clone();
            path.push(name);
            File::open(path)
        }
    }
}
