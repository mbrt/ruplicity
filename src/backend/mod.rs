//! Operations on backup files, abstracted over transport.
//!
//! This sub-module exposes a trait used to provide access to backup files, abstracting over the
//! actual transport. This could be a local mounted file system directory, a network drive, or a
//! cloud service.
pub mod local;

use std::io::{self, Read};
use std::path::Path;


/// A trait used to provide access to backup files.
pub trait Backend {
    /// A file name. It must be convertible to a `Path`.
    type FileName: AsRef<Path>;

    /// An iterator over filenames.
    type FileNameIter: IntoIterator<Item=Self::FileName>;

    /// A file managed by the backend. It must implement the `Read` trait.
    type FileStream: Read;

    /// Returns a list of available file names.
    ///
    /// The file names returned should have an extension, and do not contain the base path.
    fn file_names(&self) -> io::Result<Self::FileNameIter>;

    /// Opens a file for reading.
    fn open_file(&self, name: &Path) -> io::Result<Self::FileStream>;
}
