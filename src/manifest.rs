//! Operations on manifest files.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};
use std::str::{self, FromStr};
use std::string::FromUtf8Error;
use std::usize;


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
    start_block: Option<usize>,
    end_block: Option<usize>,
    hash_type: String,
    hash: Vec<u8>,
}

/// wip
#[derive(Debug)]
pub enum ParseError {
    /// wip
    Io(io::Error),
    /// wip
    MissingKeyword(String),
    /// wip
    Utf8Error(FromUtf8Error),
}


struct ManifestParser<R> {
    input: R,
    buf: Vec<u8>,
    hostname: String,
    local_dir: Vec<u8>,
    volumes: Vec<Option<Volume>>,
}

struct LineReader<'a, R: 'a>(&'a mut R);


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
            ParseError::MissingKeyword(_) => "missing keyword in manifest",
            ParseError::Utf8Error(ref err) => err.description(),
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match *self {
            ParseError::Io(ref e) => write!(fmt, "{}", e),
            ParseError::MissingKeyword(ref e) => write!(fmt, "missing keyword '{}' in manifest", e),
            ParseError::Utf8Error(ref e) => write!(fmt, "{}", e),
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> ParseError {
        ParseError::Io(err)
    }
}

impl From<FromUtf8Error> for ParseError {
    fn from(err: FromUtf8Error) -> ParseError {
        ParseError::Utf8Error(err)
    }
}


impl<R: BufRead> ManifestParser<R> {
    pub fn new(input: R) -> Self {
        ManifestParser {
            input: input,
            buf: vec![],
            hostname: String::new(),
            local_dir: vec![],
            volumes: vec![],
        }
    }

    pub fn parse(mut self) -> Result<Manifest, ParseError> {
        self.hostname = try!(self.read_param_str("Hostname"));
        self.local_dir = try!(self.read_param_bytes("Localdir"));

        while let Some((vol, i)) = try!(self.read_volume()) {
            // resize volumes if necessary
            if i >= self.volumes.len() {
                self.volumes.reserve(i + 1);
                for _ in self.volumes.len()..i + 1 {
                    self.volumes.push(None);
                }
            }
            self.volumes[i] = Some(vol);
        }

        // make result
        unimplemented!()
    }

    fn read_volume(&mut self) -> Result<Option<(Volume, usize)>, ParseError> {
        // volume number
        let mut param = try!(self.read_param_str("Volume"));
        if param.ends_with(":") {
            param.pop();
        }
        let num = usize::from_str(&param);

        unimplemented!()
    }

    fn read_param_bytes(&mut self, key: &str) -> Result<Vec<u8>, ParseError> {
        try!(self.consume_whitespace());
        if !try!(self.consume_keyword(key)) {
            return Err(ParseError::MissingKeyword(key.to_owned()));
        }
        try!(self.consume_whitespace());
        match self.read_param_value() {
            Ok(res) => {
                try!(self.consume_newline());
                Ok(res)
            }
            Err(e) => Err(From::from(e)),
        }
    }

    fn read_param_str(&mut self, key: &str) -> Result<String, ParseError> {
        let bytes = try!(self.read_param_bytes(key));
        String::from_utf8(bytes).map_err(From::from)
    }

    fn consume_keyword(&mut self, key: &str) -> io::Result<bool> {
        try!(self.input.read_until(b' ', &mut self.buf));
        Ok(match_keyword(&self.buf, key))
    }

    fn consume_newline(&mut self) -> io::Result<()> {
        self.consume_until(|b| {
            match b {
                b' ' | b'\t' | b'\r' | b'\n' => true,
                _ => false,
            }
        })
    }

    fn consume_whitespace(&mut self) -> io::Result<()> {
        self.consume_until(|b| {
            match b {
                b' ' | b'\t' => true,
                _ => false,
            }
        })
    }

    fn consume_until<F>(&mut self, mut f: F) -> io::Result<()>
        where F: FnMut(u8) -> bool
    {
        loop {
            let (pos, end) = {
                let buf = try!(self.input.fill_buf());
                match buf.iter().cloned().position(&mut f) {
                    Some(p) => (p, true),
                    None => (buf.len(), buf.is_empty()),
                }
            };
            self.input.consume(pos);
            if end {
                return Ok(());
            }
        }
    }

    fn read_param_value(&mut self) -> io::Result<Vec<u8>> {
        if try!(self.consume_byte(b'"')) {
            try!(self.input.read_until(b'"', &mut self.buf));
        } else {
            try!(self.input.read_until(b'\n', &mut self.buf));
        }
        let mut result = Vec::with_capacity(self.buf.len());
        // unescape
        for (i, b) in self.buf.iter().cloned().enumerate() {
            if b != b'\\' {
                result.push(b);
            } else {
                // expects a \xNN where NN is a number string representing the escaped char in hex
                // e.g. \x20 is the space ' '
                if self.buf.len() - i >= 4 && self.buf[i + 1] == b'x' {
                    let num = (self.buf[i + 2] - b'0') << 4 + self.buf[i + 3] - b'0';
                    result.push(num);
                }
            }
        }
        Ok(result)
    }

    fn consume_byte(&mut self, expected: u8) -> io::Result<bool> {
        let found = {
            let buf = try!(self.input.fill_buf());
            buf.first().map_or(false, |b| *b == expected)
        };
        if found {
            self.input.consume(1);
        }
        Ok(found)
    }
}



impl<'a, R: BufRead + 'a> BufRead for LineReader<'a, R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let buf = try!(self.0.fill_buf());
        Ok(filter_newline(buf))
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt);
    }
}

impl<'a, R: Read + 'a> Read for LineReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = try!(self.0.read(buf));
        Ok(filter_newline(&buf[0..size]).len())
    }
}


#[inline]
fn match_keyword(buf: &[u8], key: &str) -> bool {
    str::from_utf8(&buf).ok().map_or(false, |s| s == key)
}

fn filter_newline<'b>(buf: &[u8]) -> &[u8] {
    match buf.iter().cloned().position(|b| b == b'\n') {
        Some(pos) => buf.split_at(pos).0,
        None => buf,
    }
}
