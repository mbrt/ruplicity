//! Operations on manifest files.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead, BufReader};
use std::num::ParseIntError;
use std::path::Path;
use std::result;
use std::slice;
use std::str::{self, FromStr, Utf8Error};
use std::usize;

use backend::Backend;
use collections::{BackupChain, BackupSet};
use rawpath::RawPath;


/// wip
pub type Result<T> = result::Result<T, ParseError>;

/// wip
pub type ManifestIter<'a> = slice::Iter<'a, Manifest>;


/// wip
pub struct ManifestChain {
    manifests: Vec<Manifest>,
}

/// Manifest file info.
#[derive(Debug, Eq, PartialEq)]
pub struct Manifest {
    hostname: String,
    local_dir: RawPath,
    volumes: Vec<Volume>,
}

/// Volume info.
#[derive(Debug, Eq, PartialEq)]
pub struct Volume {
    start_path: PathBlock,
    end_path: PathBlock,
    hash_type: String,
    hash: Vec<u8>,
}

/// Enumeration of parsing errors.
#[derive(Debug)]
pub enum ParseError {
    /// An IO error.
    Io(io::Error),
    /// A keyword was missing.
    MissingKeyword(String),
    /// A volume hash was missing.
    MissingHash,
    /// A volume hash type was missing.
    MissingHashType,
    /// A path was missing.
    MissingPath,
    /// The list of volumes is not sorted.
    OutOfOrderVolume(usize),
    /// An integer parsing error.
    ParseInt(ParseIntError),
    /// An error parsing an UTF-8 string.
    Utf8(Utf8Error),
}


#[derive(Debug, Eq, PartialEq)]
struct PathBlock {
    path: RawPath,
    block: Option<usize>,
}

struct ManifestParser<R> {
    input: R,
    buf: Vec<u8>,
}

struct WordIter<'a>(&'a [u8]);


impl ManifestChain {
    /// wip
    pub fn from_backup_chain<B: Backend>(backend: &B, chain: &BackupChain) -> Result<Self> {
        let work = |set: &BackupSet| {
            let path = Path::new(set.manifest_path());
            let mut file = BufReader::new(try!(backend.open_file(path)));
            Manifest::parse(&mut file)
        };

        let mut result = Vec::new();
        result.push(try!(work(chain.full_set())));
        for set in chain.inc_sets() {
            result.push(try!(work(set)));
        }

        Ok(ManifestChain { manifests: result })
    }

    /// wip
    pub fn iter(&self) -> ManifestIter {
        self.into_iter()
    }

    /// wip
    pub fn get(&self, index: usize) -> Option<&Manifest> {
        self.manifests.get(index)
    }
}

impl<'a> IntoIterator for &'a ManifestChain {
    type Item = &'a Manifest;
    type IntoIter = ManifestIter<'a>;

    fn into_iter(self) -> ManifestIter<'a> {
        self.manifests.iter()
    }
}


impl Manifest {
    /// Parses a stream to get a manifest.
    pub fn parse<R: BufRead>(m: &mut R) -> Result<Self> {
        let parser = ManifestParser::new(m);
        parser.parse()
    }

    /// The hostname produced the backup.
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    /// The original backup root path.
    pub fn local_dir(&self) -> Option<&Path> {
        self.local_dir.as_path()
    }

    /// The index of the last volume.
    pub fn last_volume_index(&self) -> usize {
        self.volumes.len()
    }

    /// Returns the volume corresponding to the given index if present.
    ///
    /// Note that volumes are counting starting from one, so the last volume number is equal to the
    /// number of volumes. If no volume corresponds to the given number, `None` is returned.
    pub fn volume(&self, num: usize) -> Option<&Volume> {
        if num == 0 {
            None
        } else {
            self.volumes.get(num - 1)
        }
    }

    /// Returns the index of the first volume containing the given path, if present.
    ///
    /// The given path is represented with a byte array, because:
    ///
    /// * duplicity supports non-UTF8 paths;
    /// * under Windows `Path` is not allowed to contain non-UTF8 sequences.
    pub fn first_volume_of_path(&self, path: &[u8]) -> Option<usize> {
        self.volumes
            .binary_search_by(|v| {
                match path.cmp(v.start_path_bytes()) {
                    Ordering::Less => Ordering::Greater,
                    Ordering::Greater => {
                        match path.cmp(v.end_path_bytes()) {
                            Ordering::Less | Ordering::Equal => Ordering::Equal,
                            Ordering::Greater => Ordering::Less,
                        }
                    }
                    Ordering::Equal => {
                        match v.start_path.block {
                            Some(n) if n > 0 => Ordering::Greater,
                            _ => Ordering::Equal,
                        }
                    }
                }
            })
            .map(|idx| idx + 1)
            .ok()
    }

    /// Returns the index of the last volume containing the given path, if present.
    ///
    /// The given path is represented with a byte array, because:
    ///
    /// * duplicity supports non-UTF8 paths;
    /// * under Windows `Path` is not allowed to contain non-UTF8 sequences.
    pub fn last_volume_of_path(&self, path: &[u8]) -> Option<usize> {
        self.volumes
            .binary_search_by(|v| {
                match path.cmp(v.end_path_bytes()) {
                    Ordering::Greater => Ordering::Less,
                    Ordering::Less => {
                        match path.cmp(v.start_path_bytes()) {
                            Ordering::Greater | Ordering::Equal => Ordering::Equal,
                            Ordering::Less => Ordering::Greater,
                        }
                    }
                    Ordering::Equal => {
                        if v.end_path.block.is_some() {
                            Ordering::Less
                        } else {
                            Ordering::Equal
                        }
                    }
                }
            })
            .map(|idx| idx + 1)
            .ok()
    }

    /// wip
    pub fn volume_of_block(&self, path: &[u8], block: usize) -> Option<usize> {
        unimplemented!()
    }
}


impl Volume {
    /// Returns the first path handled by this volume.
    ///
    /// Note that the path can be `None` under Windows, in case it is non-UTF8. Use
    /// `start_path_bytes` if you need the byte array representing the path. This function never
    /// fails under *nix systems.
    pub fn start_path(&self) -> Option<&Path> {
        self.start_path.path.as_path()
    }

    /// Returns the last path handled by this volume.
    ///
    /// Note that the path can be `None` under Windows, in case it is non-UTF8. Use
    /// `start_path_bytes` if you need the byte array representing the path. This function never
    /// fails under *nix systems.
    pub fn end_path(&self) -> Option<&Path> {
        self.end_path.path.as_path()
    }

    /// Returns the first path handled by this volume, represented as a byte array.
    pub fn start_path_bytes(&self) -> &[u8] {
        self.start_path.path.as_bytes()
    }

    /// Returns the first path handled by this volume, represented as a byte array.
    pub fn end_path_bytes(&self) -> &[u8] {
        self.end_path.path.as_bytes()
    }

    /// Returns the number of the starting block of the first path.
    ///
    /// If the first path is divided in multiple blocks and this volume does not start with the
    /// first block of that path, this function returns that block number.
    pub fn start_block(&self) -> Option<usize> {
        self.start_path.block
    }

    /// Returns the number of the last block of the last path.
    ///
    /// If the last path is divided in multiple blocks and this volume does not ends with the
    /// last block of that path, this function returns that block number.
    pub fn end_block(&self) -> Option<usize> {
        self.end_path.block
    }

    /// Returns a string representing the hash type of this volume.
    pub fn hash_type(&self) -> &str {
        &self.hash_type
    }

    /// Returns the hash value of this volume.
    pub fn hash(&self) -> &[u8] {
        &self.hash
    }
}


impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::Io(ref err) => err.description(),
            ParseError::MissingKeyword(_) => "missing keyword in manifest",
            ParseError::MissingHash => "missing required hash",
            ParseError::MissingHashType => "missing required hash type",
            ParseError::MissingPath => "missing required path",
            ParseError::OutOfOrderVolume(_) => "a volume is missing or volumes are unsorted",
            ParseError::ParseInt(ref err) => err.description(),
            ParseError::Utf8(ref err) => err.description(),
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match *self {
            ParseError::Io(ref e) => write!(fmt, "{}", e),
            ParseError::MissingKeyword(ref e) => write!(fmt, "missing keyword '{}' in manifest", e),
            ParseError::OutOfOrderVolume(v) => {
                write!(fmt, "volumes are not sorted around volume {}", v)
            }
            ParseError::ParseInt(ref e) => write!(fmt, "{}", e),
            ParseError::Utf8(ref e) => write!(fmt, "{}", e),
            _ => write!(fmt, "{}", self.description()),
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> ParseError {
        ParseError::Io(err)
    }
}

impl From<ParseIntError> for ParseError {
    fn from(err: ParseIntError) -> ParseError {
        ParseError::ParseInt(err)
    }
}

impl From<Utf8Error> for ParseError {
    fn from(err: Utf8Error) -> ParseError {
        ParseError::Utf8(err)
    }
}


macro_rules! check_eof(
    ($e:expr) => (
        if !try!($e) {
            return Err(From::from(io::Error::new(io::ErrorKind::UnexpectedEof,
                                                 "file ends unexpectedly")));
        }
    )
);


impl<R: BufRead> ManifestParser<R> {
    pub fn new(input: R) -> Self {
        ManifestParser {
            input: input,
            buf: vec![],
        }
    }

    pub fn parse(mut self) -> Result<Manifest> {
        check_eof!(self.read_line());
        let hostname = try!(self.read_param_str("Hostname"));
        check_eof!(self.read_line());
        let local_dir = RawPath::from_bytes(try!(self.read_param_bytes("Localdir")));

        let mut volumes = Vec::new();
        while let Some((vol, i)) = try!(self.read_volume()) {
            // check if out of order
            if i != volumes.len() + 1 {
                return Err(ParseError::OutOfOrderVolume(i));
            }
            volumes.push(vol);
        }

        Ok(Manifest {
            hostname: hostname,
            local_dir: local_dir,
            volumes: volumes,
        })
    }

    fn read_volume(&mut self) -> Result<Option<(Volume, usize)>> {
        if !try!(self.read_line()) {
            // EOF
            return Ok(None);
        }

        // volume number
        let mut param = try!(self.read_param_str("Volume"));
        if param.ends_with(':') {
            param.pop();
        }
        let num = try!(usize::from_str(&param));
        check_eof!(self.read_line());
        let start_path = try!(self.read_path_block("StartingPath"));
        check_eof!(self.read_line());
        let end_path = try!(self.read_path_block("EndingPath"));
        check_eof!(self.read_line());
        let (htype, h) = try!(self.read_hash_param());

        let vol = Volume {
            start_path: start_path,
            end_path: end_path,
            hash_type: htype,
            hash: h,
        };
        Ok(Some((vol, num)))
    }

    fn read_line(&mut self) -> io::Result<bool> {
        self.buf.clear();
        let mut len = try!(self.input.read_until(b'\n', &mut self.buf));
        if len > 0 && self.buf[len - 1] == b'\n' {
            len -= 1;
        }
        if len > 0 && self.buf[len - 1] == b'\r' {
            len -= 1;
        }
        self.buf.truncate(len);

        Ok(!self.buf.is_empty())
    }

    fn read_param_bytes(&mut self, key: &str) -> Result<Vec<u8>> {
        let mut words = WordIter(&self.buf);
        let kw = match words.next() {
            Some(word) => try!(str::from_utf8(word)),
            None => "",
        };
        if kw != key {
            return Err(ParseError::MissingKeyword(key.to_owned()));
        }
        let param = match words.next() {
            Some(word) => word,
            None => {
                return Ok(vec![]);
            }
        };
        Ok(unescape(param))
    }

    fn read_param_str(&mut self, key: &str) -> Result<String> {
        let bytes = try!(self.read_param_bytes(key));
        String::from_utf8(bytes).map_err(|e| From::from(e.utf8_error()))
    }

    fn read_path_block(&mut self, key: &str) -> Result<PathBlock> {
        let mut words = WordIter(&self.buf);
        let kw = match words.next() {
            Some(word) => try!(str::from_utf8(word)),
            None => "",
        };
        if kw != key {
            return Err(ParseError::MissingKeyword(key.to_owned()));
        }
        let path = match words.next() {
            Some(word) => RawPath::from_bytes(unescape(word)),
            None => {
                return Err(ParseError::MissingPath);
            }
        };
        let block = match words.next() {
            Some(word) => {
                let s = try!(str::from_utf8(word));
                Some(try!(usize::from_str(s)))
            }
            None => None,
        };

        Ok(PathBlock {
            path: path,
            block: block,
        })
    }

    fn read_hash_param(&mut self) -> Result<(String, Vec<u8>)> {
        let mut words = WordIter(&self.buf);
        let kw = match words.next() {
            Some(word) => try!(str::from_utf8(word)),
            None => "",
        };
        if kw != "Hash" {
            return Err(ParseError::MissingKeyword("Hash".to_owned()));
        }
        let htype = match words.next() {
            Some(word) => try!(str::from_utf8(word)).to_owned(),
            None => {
                return Err(ParseError::MissingHashType);
            }
        };
        let hash = match words.next() {
            Some(word) => from_hex(word),
            None => {
                return Err(ParseError::MissingHash);
            }
        };

        Ok((htype, hash))
    }
}


impl<'a> Iterator for WordIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.0.is_empty() {
                return None;
            }
            let pos = self.0.iter().position(|b| *b == b' ').unwrap_or(self.0.len());
            let (w, rest) = {
                let (w, rest) = self.0.split_at(pos);
                // skip all the spaces from rest
                let pos = rest.iter().position(|b| *b != b' ').unwrap_or(rest.len());
                (w, &rest[pos..])
            };
            self.0 = rest;
            if !w.is_empty() {
                return Some(w);
            }
        }
    }
}


fn unescape(mut buf: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(buf.len());
    buf = match (buf.first().cloned(), buf.last().cloned()) {
        // quoted
        (Some(b'"'), _) if buf.len() > 1 => &buf[1..buf.len() - 1],
        // unquoted or invalid single "
        (Some(_), _) => buf,
        // empty
        _ => {
            return result;
        }
    };

    // unescape
    let mut i = 0;
    let len = buf.len();
    while i < len {
        let b = buf[i];
        if b != b'\\' {
            result.push(b);
        } else if buf.len() - i >= 4 && buf[i + 1] == b'x' {
            // expects a \xNN where NN is a number string representing the escaped char in hex
            // e.g. \x20 is the space ' '
            let num = (nibble(buf[i + 2]) << 4) | nibble(buf[i + 3]);
            result.push(num);
            i += 3;
        }
        // otherwise ignore
        i += 1;
    }

    result
}

fn from_hex(s: &[u8]) -> Vec<u8> {
    let mut res = Vec::with_capacity(s.len() / 2);
    let mut buf: u8 = 0;

    for (idx, byte) in s.iter().cloned().enumerate() {
        buf <<= 4;
        buf |= nibble(byte);

        if idx % 2 == 1 {
            res.push(buf);
            buf = 0;
        }
    }
    res
}

fn nibble(b: u8) -> u8 {
    match b {
        b'a'...b'f' => b - b'a' + 10,
        b'0'...b'9' => b - b'0',
        _ => 0,
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;

    use backend::Backend;
    use backend::local::LocalBackend;
    use collections::Collections;


    fn full1_manifest() -> Result<Manifest> {
        let file = File::open("tests/manifest/full1.manifest").unwrap();
        let mut bfile = BufReader::new(file);
        Manifest::parse(&mut bfile)
    }

    fn inc1_manifest() -> Result<Manifest> {
        let file = File::open("tests/manifest/inc1.manifest").unwrap();
        let mut bfile = BufReader::new(file);
        Manifest::parse(&mut bfile)
    }


    #[test]
    fn parse_no_err_full() {
        full1_manifest().unwrap();
    }

    #[test]
    fn parse_no_err_inc() {
        inc1_manifest().unwrap();
    }

    #[test]
    fn first_volume_of_path() {
        let manifest = inc1_manifest().unwrap();
        assert_eq!(manifest.first_volume_of_path(b"home/michele/Immagini/Foto/albumfiles.txt")
                           .unwrap(),
                   28);
        assert_eq!(manifest.first_volume_of_path(b"home/michele/Documenti/Scuola/Open Class/\
                                                 Epfl/Principles of Reactive Programming/lectures/\
                                                 week7/lecture_slides_week7-1-annotated.pdf")
                           .unwrap(),
                   1);
        assert_eq!(manifest.first_volume_of_path(b"home/michele/Documenti/Scuola/Uni/\
                                                 Calcolo Numerico/octave docs/tutorial.pdf")
                           .unwrap(),
                   18);
    }

    #[test]
    fn last_volume_of_path() {
        let manifest = inc1_manifest().unwrap();
        assert_eq!(manifest.last_volume_of_path(b"home/michele/Immagini/Foto/albumfiles.txt")
                           .unwrap(),
                   28);
        assert_eq!(manifest.last_volume_of_path(b"home/michele/Documenti/Scuola/Open Class/\
                                                Epfl/Principles of Reactive Programming/lectures/\
                                                week7/lecture_slides_week7-1-annotated.pdf")
                           .unwrap(),
                   2);
        assert_eq!(manifest.last_volume_of_path(b"home/michele/Documenti/Scuola/Uni/\
                                                Calcolo Numerico/octave docs/tutorial.pdf")
                           .unwrap(),
                   19);
        assert_eq!(manifest.last_volume_of_path(b"home/michele/Immagini/Foto/foto1.jpg"),
                   None);
    }

    #[test]
    fn full1_data() {
        let manifest = full1_manifest().unwrap();
        assert_eq!(manifest.hostname(), "dellxps");
        assert_eq!(manifest.local_dir().unwrap(), Path::new("dir1"));
        assert_eq!(manifest.last_volume_index(), 1);
        let vol = manifest.volume(1).unwrap();
        assert_eq!(vol.start_path().unwrap(), Path::new("."));
        let path = vec![0xd8, 0xab, 0xb1, 0x57, 0x62, 0xae, 0xc5, 0x5d, 0x8a, 0xbb, 0x15, 0x76,
                        0x2a, 0xf4, 0x0f, 0x21, 0xf9, 0x3e, 0xe2, 0x59, 0x86, 0xbb, 0xab, 0xdb,
                        0x70, 0xb0, 0x84, 0x13, 0x6b, 0x1d, 0xc2, 0xf1, 0xf5, 0x65, 0xa5, 0x55,
                        0x82, 0x9a, 0x55, 0x56, 0xa0, 0xf4, 0xdf, 0x34, 0xba, 0xfd, 0x58, 0x03,
                        0x82, 0x07, 0x73, 0xce, 0x9e, 0x8b, 0xb3, 0x34, 0x04, 0x9f, 0x17, 0x20,
                        0xf4, 0x8f, 0xa6, 0xfa, 0x97, 0xab, 0xd8, 0xac, 0xda, 0x85, 0xdc, 0x4b,
                        0x76, 0x43, 0xfa, 0x23, 0x94, 0x92, 0x9e, 0xc9, 0xb7, 0xc3, 0x5f, 0x0f,
                        0x84, 0x67, 0x9a, 0x42, 0x11, 0x3c, 0x3d, 0x5e, 0xdb, 0x4d, 0x13, 0x96,
                        0x63, 0x8b, 0xa7, 0x7c, 0x2a, 0x22, 0x5c, 0x27, 0x5e, 0x24, 0x40, 0x23,
                        0x21, 0x28, 0x29, 0x7b, 0x7d, 0x3f, 0x2b, 0x20, 0x7e, 0x60, 0x20];
        assert_eq!(vol.end_path_bytes().to_vec(), path);
        assert_eq!(vol.hash_type(), "SHA1");
        let hash = vec![0xe4, 0xa2, 0xe8, 0xe2, 0xab, 0xfb, 0xa2, 0xcb, 0x24, 0x77, 0x2e, 0x5f,
                        0xf9, 0xda, 0x4b, 0x85, 0xb3, 0xc1, 0x9a, 0x0c];
        assert_eq!(vol.hash().to_vec(), hash);
    }

    #[test]
    fn manifests() {
        let backend = LocalBackend::new("tests/backups/multi_chain");
        let filenames = backend.file_names().unwrap();
        let coll = Collections::from_filenames(filenames);
        assert_eq!(coll.num_snapshots(), 4);
        for chain in coll.backup_chains() {
            let manifests = ManifestChain::from_backup_chain(&backend, &chain).unwrap();
            assert!(manifests.iter().count() > 0);
        }
    }
}
