pub use self::os::RawPath;


#[cfg(unix)]
mod os {
    use std::path::{Path, PathBuf};
    use std::os::unix::prelude::*;
    use std::ffi::OsString;

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct RawPath(PathBuf);


    impl RawPath {
        #[allow(dead_code)]
        pub fn new() -> Self {
            Self::from_bytes(vec![])
        }

        pub fn from_bytes(bytes: Vec<u8>) -> Self {
            RawPath(PathBuf::from(OsString::from_vec(bytes)))
        }

        pub fn as_path(&self) -> Option<&Path> {
            Some(&self.0)
        }

        pub fn as_bytes(&self) -> &[u8] {
            self.0.as_os_str().as_bytes()
        }
    }
}


#[cfg(windows)]
mod os {
    use std::cmp::{Ordering, PartialOrd};
    use std::path::{Path, PathBuf};
    use std::str;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum RawPath {
        Path(PathBuf),
        Bytes(Vec<u8>),
    }


    impl RawPath {
        #[allow(dead_code)]
        pub fn new() -> Self {
            RawPath::Bytes(vec![])
        }

        pub fn from_bytes(bytes: Vec<u8>) -> Self {
            if let Ok(s) = str::from_utf8(&bytes) {
                return RawPath::Path(PathBuf::from(s));
            }
            RawPath::Bytes(bytes)
        }

        pub fn as_path(&self) -> Option<&Path> {
            match *self {
                RawPath::Path(ref p) => Some(&p),
                RawPath::Bytes(_) => None,
            }
        }

        pub fn as_bytes(&self) -> &[u8] {
            match *self {
                RawPath::Path(ref p) => p.as_os_str().to_str().unwrap().as_bytes(),
                RawPath::Bytes(ref b) => b,
            }
        }
    }

    impl PartialOrd<RawPath> for RawPath {
        fn partial_cmp(&self, other: &RawPath) -> Option<Ordering> {
            self.as_bytes().partial_cmp(other.as_bytes())
        }
    }

    impl Ord for RawPath {
        fn cmp(&self, other: &RawPath) -> Ordering {
            self.as_bytes().cmp(other.as_bytes())
        }
    }
}
