use std::borrow::Cow;
use std::cmp;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use tar::{File, Header};

// TODO #25: Replace this function with a call to header.link_name() when tar crate is updated.
pub fn link_name(header: &Header) -> io::Result<Option<Cow<Path>>> {
    match link_name_bytes(header) {
        Some(bytes) => bytes2path(bytes).map(Some),
        None => Ok(None),
    }
}

// TODO #25: Replace this struct when tar::GnuEntries will be published.
pub struct GnuEntries<I>(I);

pub struct GnuEntry<'a, R: 'a> {
    inner: File<'a, R>,
    name: Option<Vec<u8>>,
}


impl<'a, I, R> GnuEntries<I>
    where I: Iterator<Item = io::Result<File<'a, R>>>,
          R: Read + 'a
{
    pub fn new(i: I) -> Self {
        GnuEntries(i)
    }
}

impl<'a, I, R> Iterator for GnuEntries<I>
    where I: Iterator<Item = io::Result<File<'a, R>>>,
          R: Read + 'a
{
    type Item = io::Result<GnuEntry<'a, R>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut entry = match self.0.next() {
            Some(Ok(e)) => e,
            Some(Err(e)) => {
                return Some(Err(e));
            }
            None => return None,
        };
        if entry.header().link[0] != b'L' {
            return Some(Ok(GnuEntry {
                inner: entry,
                name: None,
            }));
        }
        // Don't allow too too crazy allocation sizes up front
        let cap = cmp::min(entry.header().size().unwrap_or(0), 128 * 1024) as usize;
        let mut filename = Vec::with_capacity(cap);
        filename.resize(cap, 0);
        match entry.read(&mut filename) {
            Ok(e) => e,
            Err(e) => {
                return Some(Err(e));
            }
        };
        // fix leading \0 if present
        if let Some(pos) = filename.iter().rposition(|c| *c == 0) {
            filename.resize(pos, 0);
        }
        while let Some(c) = filename.last().cloned() {
            if c == 0 {
                filename.pop();
            } else {
                break;
            }
        }
        match self.0.next() {
            Some(Ok(e)) => {
                Some(Ok(GnuEntry {
                    inner: e,
                    name: Some(filename),
                }))
            }
            Some(Err(e)) => Some(Err(e)),
            None => Some(Err(other("longname entry not followed by another"))),
        }
    }
}


impl<'a, R: 'a + Read> GnuEntry<'a, R> {
    /// Returns access to the header of this entry in the archive.
    pub fn header(&self) -> &Header {
        self.inner.header()
    }

    pub fn path(&self) -> io::Result<Cow<Path>> {
        match self.name {
            Some(ref bytes) => bytes2path(Cow::Borrowed(bytes)),
            None => self.header().path(),
        }
    }
}


impl<'a, R: Read> Read for GnuEntry<'a, R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.inner.read(into)
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

fn truncate(slice: &[u8]) -> &[u8] {
    match slice.iter().position(|i| *i == 0) {
        Some(i) => &slice[..i],
        None => slice,
    }
}

#[cfg(windows)]
fn bytes2path(bytes: Cow<[u8]>) -> io::Result<Cow<Path>> {
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

fn other(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}


#[cfg(test)]
mod test {
    use super::*;
    use tar::Archive;
    use std::fs::File;

    #[test]
    fn long_path() {
        let file = File::open("tests/long_path.tar").unwrap();
        let mut tar = Archive::new(file);
        let last_entry = GnuEntries::new(tar.files_mut().unwrap()).last().unwrap().unwrap();
        let path = last_entry.path().unwrap();
        assert_eq!(path.to_str().unwrap(),
                   "home/michele/Documenti/Development/Progetti/MetaCloudExperiment\
                   /Reference/duplicati/BuildTools/WixIncludeMake/Program.cs");
    }
}
