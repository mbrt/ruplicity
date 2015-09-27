use std::io;

/// Backend is a trait used to provide access to backup files.
pub trait Backend {
    /// FileName is an associated type for a file name. It must be convertible to a string
    /// reference.
    type FileName: AsRef<str>;

    /// Returns a list of available file names.
    /// The file names returned should have an extension, and not a path.
    fn get_file_names(&self) -> io::Result<&[Self::FileName]>;
}


pub mod local {
    use super::Backend;
    use std::path::Path;
    use std::io;
    use std::fs;


    /// Backend operating on the local filesystem.
    pub struct LocalBackend {
        file_names: Vec<String>
    }

    impl LocalBackend {
        /// Create a new LocalBackend that operates on the given directory.
        pub fn new<T: AsRef<Path>>(path: T) -> io::Result<Self> {
            let paths = try!(fs::read_dir(path));
            let mut filenames = Vec::new();
            for entry in paths {
                let entry = unwrap_or_continue!(entry);
                let filename = unwrap_or_continue!(entry.file_name().into_string());
                filenames.push(filename);
            }
            Ok(LocalBackend{ file_names: filenames })
        }
    }

    impl Backend for LocalBackend {
        type FileName = String;

        fn get_file_names(&self) -> io::Result<&[String]> {
            Ok(&self.file_names)
        }
    }
}
