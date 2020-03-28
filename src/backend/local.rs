//! Local file system backend.
//!
//! This sub-module provides types for accessing the local file system as a backend.
//!
//! # Example
//!
//! ```
//! use ruplicity::backend::Backend;
//! use ruplicity::backend::local::LocalBackend;
//! use std::io::Read;
//! use std::path::Path;
//!
//! let backend = LocalBackend::new("tests/backend");
//! for file in backend.file_names().unwrap() {
//!     // print the current path
//!     let path: &Path = file.as_ref();
//!     println!("file: {}", path.to_str().unwrap());
//!     // print file contents
//!     let mut file = backend.open_file(path).unwrap();
//!     let mut contents = Vec::new();
//!     file.read_to_end(&mut contents).unwrap();
//!     println!("contents: {}", String::from_utf8(contents).unwrap());
//! }
//! ```

use super::Backend;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

/// Backend for some directory in the local file system.
#[derive(Debug)]
pub struct LocalBackend {
    base_path: PathBuf,
}

/// Iterator over a set of file names.
pub struct FileNameIterator(fs::ReadDir);

impl LocalBackend {
    /// Creates a new local backend for the given directory.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        LocalBackend {
            base_path: path.as_ref().to_path_buf(),
        }
    }
}

impl Backend for LocalBackend {
    type FileName = OsString;
    type FileNameIter = FileNameIterator;
    type FileStream = File;

    fn file_names(&self) -> io::Result<Self::FileNameIter> {
        let dir = fs::read_dir(self.base_path.as_path())?;
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

#[cfg(test)]
mod test {
    use super::*;
    use backend::Backend;

    #[test]
    fn multi_chain_files() {
        let backend = LocalBackend::new("tests/backups/multi_chain");
        let files = backend.file_names().unwrap().collect::<Vec<_>>();
        let actual = {
            let mut r = files
                .iter()
                .map(|p| p.to_str().unwrap())
                .filter(|p| p.starts_with("duplicity-"))
                .collect::<Vec<_>>();
            r.sort();
            r
        };
        let expected = vec![
            "duplicity-full-signatures.20160108T223144Z.sigtar.gz",
            "duplicity-full-signatures.20160108T223209Z.sigtar.gz",
            "duplicity-full.20160108T223144Z.manifest",
            "duplicity-full.20160108T223144Z.vol1.difftar.gz",
            "duplicity-full.20160108T223209Z.manifest",
            "duplicity-full.20160108T223209Z.vol1.difftar.gz",
            "duplicity-inc.20160108T223144Z.to.20160108T223159Z.manifest",
            "duplicity-inc.20160108T223144Z.to.20160108T223159Z.vol1.difftar.gz",
            "duplicity-inc.20160108T223209Z.to.20160108T223217Z.manifest",
            "duplicity-inc.20160108T223209Z.to.20160108T223217Z.vol1.difftar.gz",
            "duplicity-new-signatures.20160108T223144Z.to.20160108T223159Z.sigtar.gz",
            "duplicity-new-signatures.20160108T223209Z.to.20160108T223217Z.sigtar.gz",
        ];
        assert_eq!(actual, expected);
    }
}
