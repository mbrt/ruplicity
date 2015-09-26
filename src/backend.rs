use std::io;
use std::result;


pub trait Backend {
    fn get_file_names(&self) -> &[String];
}


pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    UnknownProtocol,
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}


pub fn make_backend_from_url(url: &str) -> Result<Box<Backend>> {
    if url.starts_with("file://") {
        // TODO split??
        //let (_, path) = url.split_at(7);
        let backend = try!(local::LocalBackend::new(url));
        Ok(Box::new(backend))
    }
    else {
        Err(Error::UnknownProtocol)
    }
}


mod local {
    use super::Backend;
    use std::path::Path;
    use std::io;
    use std::fs;


    pub struct LocalBackend {
        file_names: Vec<String>
    }

    impl LocalBackend {
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
        fn get_file_names(&self) -> &[String] {
            &self.file_names
        }
    }
}
