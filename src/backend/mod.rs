//! Transport layer for backup files.
//!
//! This sub-module exposes a trait to access a file system containing duplicity backup files,
//! abstracting over the actual transport. This could be a local mounted file system directory,
//! a network drive, a cloud service, or whatever.
pub mod local;

use std::io::{self, Read};
use std::path::Path;


/// A trait used to provide a transport layer for backup files.
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
