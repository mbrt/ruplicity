use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};

use tar::Header;

// TODO #25: Replace this function with a call to header.link_name() when tar crate is updated.
pub fn link_name(header: &Header) -> io::Result<Option<Cow<Path>>> {
    match link_name_bytes(header) {
        Some(bytes) => bytes2path(bytes).map(Some),
        None => Ok(None),
    }
}

fn link_name_bytes(header: &Header) -> Option<Cow<[u8]>> {
    if header.linkname[0] == 0 { None } else { Some(deslash(&header.linkname)) }
}

fn deslash(bytes: &[u8]) -> Cow<[u8]> {
    if !bytes.contains(&b'\\') {
        Cow::Borrowed(truncate(bytes))
    } else {
        fn noslash(b: &u8) -> u8 {
            if *b == b'\\' { b'/' } else { *b }
        }
        Cow::Owned(truncate(bytes).iter().map(noslash).collect())
    }
}

fn truncate<'a>(slice: &'a [u8]) -> &'a [u8] {
    match slice.iter().position(|i| *i == 0) {
        Some(i) => &slice[..i],
        None => slice,
    }
}

#[cfg(windows)]
fn bytes2path(bytes: Cow<[u8]>) -> io::Result<Cow<Path>> {
    use std::os::windows::prelude::*;
    use std::str;

    return match bytes {
        Cow::Borrowed(bytes) => {
            let s = try!(str::from_utf8(bytes).map_err(|_| not_unicode()));
            Ok(Cow::Borrowed(Path::new(s)))
        }
        Cow::Owned(bytes) => {
            let s = try!(String::from_utf8(bytes).map_err(|_| not_unicode()));
            Ok(Cow::Owned(PathBuf::from(s)))
        }
    };

    fn not_unicode() -> io::Error {
        other("only unicode paths are supported on windows")
    }

    fn other(msg: &str) -> io::Error {
        io::Error::new(io::ErrorKind::Other, msg)
    }
}

#[cfg(unix)]
fn bytes2path(bytes: Cow<[u8]>) -> io::Result<Cow<Path>> {
    use std::ffi::{OsStr, OsString};
    use std::os::unix::prelude::*;

    Ok(match bytes {
        Cow::Borrowed(bytes) => {
            Cow::Borrowed({
                Path::new(OsStr::from_bytes(bytes))
            })
        }
        Cow::Owned(bytes) => {
            Cow::Owned({
                PathBuf::from(OsString::from_vec(bytes))
            })
        }
    })
}
