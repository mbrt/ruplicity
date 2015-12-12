pub mod local;

use std::io::{self, Read};
use std::path::Path;


/// Backend is a trait used to provide access to backup files.
pub trait Backend {
    /// FileName is an associated type for a file name. It must be convertible to a string
    /// reference.
    type FileName: AsRef<Path>;

    /// FileNameIter is an associated type for an iterator over filenames.
    type FileNameIter: IntoIterator<Item=Self::FileName>;

    /// FileStream is an associated type for a read stream for a file.
    type FileStream: Read;

    /// Returns a list of available file names.
    /// The file names returned should have an extension, and do not contain the base path.
    fn file_names(&self) -> io::Result<Self::FileNameIter>;

    /// Open a file for reading.
    fn open_file(&self, name: &Path) -> io::Result<Self::FileStream>;
}
