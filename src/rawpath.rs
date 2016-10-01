pub use self::os::{RawPath, RawPathBuf};


#[cfg(unix)]
mod os {
    use std::ffi::{OsStr, OsString};
    use std::fmt::{self, Display, Formatter};
    use std::os::unix::prelude::*;
    use std::path::{Path, PathBuf};

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct RawPath<'a>(&'a Path);

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct RawPathBuf(PathBuf);


    impl<'a> RawPath<'a> {
        pub fn new(bytes: &'a [u8]) -> Self {
            RawPath(Path::new(OsStr::from_bytes(bytes)))
        }

        pub fn as_path(&self) -> Option<&'a Path> {
            Some(&self.0)
        }

        pub fn as_bytes(&self) -> &'a [u8] {
            self.0.as_os_str().as_bytes()
        }

        pub fn as_raw_path_buf(&self) -> RawPathBuf {
            RawPathBuf::from_bytes(self.0.as_os_str().as_bytes().to_owned())
        }
    }

    impl<'a> Display for RawPath<'a> {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            if self.as_bytes().is_empty() {
                write!(f, ".")
            } else {
                match self.0.to_str() {
                    Some(s) => write!(f, "{}", s),
                    None => write!(f, "?"),
                }
            }
        }
    }


    impl RawPathBuf {
        #[allow(dead_code)]
        pub fn new() -> Self {
            Self::from_bytes(vec![])
        }

        pub fn from_bytes(bytes: Vec<u8>) -> Self {
            RawPathBuf(PathBuf::from(OsString::from_vec(bytes)))
        }

        pub fn as_path(&self) -> Option<&Path> {
            Some(&self.0)
        }

        pub fn as_bytes(&self) -> &[u8] {
            self.0.as_os_str().as_bytes()
        }

        pub fn as_raw_path(&self) -> RawPath {
            RawPath(&self.0)
        }
    }

    impl Display for RawPathBuf {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            self.as_raw_path().fmt(f)
        }
    }
}


#[cfg(windows)]
mod os {
    use std::cmp::{Ordering, PartialOrd};
    use std::fmt::{self, Display, Formatter};
    use std::path::{Path, PathBuf};
    use std::str;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum RawPath<'a> {
        Path(&'a Path),
        Bytes(&'a [u8]),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum RawPathBuf {
        Path(PathBuf),
        Bytes(Vec<u8>),
    }


    impl<'a> RawPath<'a> {
        pub fn new(bytes: &'a [u8]) -> Self {
            match str::from_utf8(&bytes) {
                Ok(s) => RawPath::Path(Path::new(s)),
                _ => RawPath::Bytes(bytes),
            }
        }

        pub fn as_path(&self) -> Option<&'a Path> {
            match *self {
                RawPath::Path(p) => Some(p),
                RawPath::Bytes(_) => None,
            }
        }

        pub fn as_bytes(&self) -> &'a [u8] {
            match *self {
                RawPath::Path(p) => p.as_os_str().to_str().unwrap().as_bytes(),
                RawPath::Bytes(b) => b,
            }
        }

        pub fn as_raw_path_buf(&self) -> RawPathBuf {
            match *self {
                RawPath::Path(p) => RawPathBuf::Path(p.to_owned()),
                RawPath::Bytes(b) => RawPathBuf::Bytes(b.to_owned()),
            }
        }
    }

    impl<'a> PartialOrd<RawPath<'a>> for RawPath<'a> {
        fn partial_cmp(&self, other: &RawPath) -> Option<Ordering> {
            self.as_bytes().partial_cmp(other.as_bytes())
        }
    }

    impl<'a> Ord for RawPath<'a> {
        fn cmp(&self, other: &RawPath) -> Ordering {
            self.as_bytes().cmp(other.as_bytes())
        }
    }

    impl<'a> Display for RawPath<'a> {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            match *self {
                RawPath::Path(ref p) => write!(f, "{}", p.as_os_str().to_str().unwrap()),
                RawPath::Bytes(_) => write!(f, "?"),
            }
        }
    }


    impl RawPathBuf {
        #[allow(dead_code)]
        pub fn new() -> Self {
            RawPathBuf::Bytes(vec![])
        }

        pub fn from_bytes(bytes: Vec<u8>) -> Self {
            // prevent 'move out of `bytes` because it's borrowed' error
            if let Ok(s) = str::from_utf8(&bytes) {
                return RawPathBuf::Path(PathBuf::from(s));
            }
            return RawPathBuf::Bytes(bytes);
        }

        pub fn as_path(&self) -> Option<&Path> {
            match *self {
                RawPathBuf::Path(ref p) => Some(&p),
                RawPathBuf::Bytes(_) => None,
            }
        }

        pub fn as_bytes(&self) -> &[u8] {
            match *self {
                RawPathBuf::Path(ref p) => p.as_os_str().to_str().unwrap().as_bytes(),
                RawPathBuf::Bytes(ref b) => b,
            }
        }

        pub fn as_raw_path(&self) -> RawPath {
            match *self {
                RawPathBuf::Path(ref p) => RawPath::Path(p),
                RawPathBuf::Bytes(ref b) => RawPath::Bytes(b),
            }
        }
    }

    impl PartialOrd<RawPathBuf> for RawPathBuf {
        fn partial_cmp(&self, other: &RawPathBuf) -> Option<Ordering> {
            self.as_bytes().partial_cmp(other.as_bytes())
        }
    }

    impl Ord for RawPathBuf {
        fn cmp(&self, other: &RawPathBuf) -> Ordering {
            self.as_bytes().cmp(other.as_bytes())
        }
    }

    impl Display for RawPathBuf {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            self.as_raw_path().fmt(f)
        }
    }
}
