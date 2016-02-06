//! Operations on manifest files.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::str;

/// Manifest file info.
pub struct Manifest {
    hostname: String,
    local_dir: PathBuf,
    volumes: Vec<Option<Volume>>,
}

/// Volume info.
pub struct Volume {
    start_path: PathBuf,
    end_path: PathBuf,
    hash_type: String,
    hash: Vec<u8>,
}

/// wip
#[derive(Debug)]
pub enum ParseError {
    /// wip
    Io(io::Error),
    /// wip
    MissingHostname,
    /// wip
    MissingLocaldir,
}


struct ManifestParser<R> {
    input: R,
    buf: Vec<u8>,
    hostname: String,
    local_dir: Option<PathBuf>,
    volumes: Vec<Option<Volume>>,
}


impl Manifest {
    /// Parses a stream to get a manifest.
    pub fn parse<R: BufRead>(m: &mut R) -> Result<Self, ParseError> {
        let parser = ManifestParser::new(m);
        parser.parse()
    }

    /// wip
    pub fn hostname(&self) -> Option<&str> {
        Some(&self.hostname)
    }

    /// wip
    pub fn local_dir(&self) -> Option<&Path> {
        Some(&self.local_dir)
    }

    /// wip
    pub fn max_vol_num(&self) -> usize {
        self.volumes.len()
    }

    /// wip
    pub fn volume(&self, num: usize) -> Option<&Volume> {
        self.volumes.get(num).and_then(|v| v.as_ref())
    }
}


impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::Io(ref err) => err.description(),
            ParseError::MissingHostname => "missing 'Hostname' keyword",
            ParseError::MissingLocaldir => "missing 'Localdir' keyword",
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match *self {
            ParseError::Io(ref e) => write!(fmt, "{}", e),
            _ => write!(fmt, "{}", self.description()),
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> ParseError {
        ParseError::Io(err)
    }
}


impl<R: BufRead> ManifestParser<R> {
    pub fn new(input: R) -> Self {
        ManifestParser {
            input: input,
            buf: vec![],
            hostname: String::new(),
            local_dir: None,
            volumes: vec![],
        }
    }

    pub fn parse(mut self) -> Result<Manifest, ParseError> {
        // parse hostname
        if !try!(self.consume_keyword("Hostname")) {
            return Err(ParseError::MissingHostname);
        }
        try!(self.consume_whitespace());
        try!(self.input.read_line(&mut self.hostname));

        // parse localdir
        if !try!(self.consume_keyword("Localdir")) {
            return Err(ParseError::MissingLocaldir);
        }
        try!(self.consume_whitespace());


        // make result
        Ok(Manifest {
            hostname: self.hostname,
            local_dir: self.local_dir.unwrap(),
            volumes: self.volumes,
        })
    }

    fn consume_keyword(&mut self, key: &str) -> io::Result<bool> {
        try!(self.input.read_until(b' ', &mut self.buf));
        Ok(match_keyword(&self.buf, key))
    }

    fn consume_whitespace(&mut self) -> io::Result<()> {
        loop {
            let (pos, end) = {
                let buf = try!(self.input.fill_buf());
                match buf.iter().cloned().position(|b| b != b' ') {
                    Some(p) => (p, false),
                    None => (buf.len(), true),
                }
            };
            self.input.consume(pos);
            if end {
                return Ok(());
            }
        }
    }
}


#[inline]
fn match_keyword(buf: &[u8], key: &str) -> bool {
    str::from_utf8(&buf).ok().map_or(false, |s| s == key)
}
