use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::os::unix::prelude::*;
#[cfg(unix)]
use std::ffi::OsString;
#[cfg(windows)]
use std::str;


pub enum RawPath {
    Path(PathBuf),
    Bytes(Vec<u8>),
}


impl RawPath {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self::from_bytes(bytes)
    }

    pub fn as_path(&self) -> Option<&Path> {
        match *self {
            RawPath::Path(ref p) => Some(&p),
            RawPath::Bytes(ref b) => None,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match *self {
            RawPath::Path(ref p) => path2bytes(p),
            RawPath::Bytes(ref b) => b,
        }
    }

    #[cfg(windows)]
    fn from_bytes(bytes: Vec<u8>) -> Self {
        if let Ok(s) = str::from_utf8(&bytes) {
            return RawPath::Path(PathBuf::from(s));
        }
        RawPath::Bytes(bytes)
    }

    #[cfg(unix)]
    fn from_bytes(bytes: Vec<u8>) -> Self {
        RawPath::Path(PathBuf::from(OsString::from_vec(bytes)))
    }
}


#[cfg(windows)]
fn path2bytes(p: &Path) -> &[u8] {
    p.as_os_str().to_str().unwrap().as_bytes()
}

#[cfg(unix)]
fn path2bytes(p: &Path) -> &[u8] {
    p.as_os_str().as_bytes()
}
