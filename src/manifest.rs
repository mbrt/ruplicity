//! Operations on manifest files.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead};
use std::num::ParseIntError;
use std::path::Path;
use std::str::{self, FromStr};
use std::string::FromUtf8Error;
use std::usize;

use rawpath::RawPath;


/// Manifest file info.
pub struct Manifest {
    hostname: String,
    local_dir: RawPath,
    volumes: Vec<Option<Volume>>,
}

/// Volume info.
pub struct Volume {
    start_path: PathBlock,
    end_path: PathBlock,
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
    ParseInt(ParseIntError),
    /// wip
    Utf8(FromUtf8Error),
}


struct PathBlock {
    path: RawPath,
    block: Option<usize>,
}

struct ManifestParser<R> {
    input: R,
    buf: Vec<u8>,
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
        self.local_dir.as_path()
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


impl Volume {
    /// wip
    pub fn start_path(&self) -> Option<&Path> {
        self.start_path.path.as_path()
    }

    /// wip
    pub fn end_path(&self) -> Option<&Path> {
        self.end_path.path.as_path()
    }

    /// wip
    pub fn start_path_bytes(&self) -> &[u8] {
        self.start_path.path.as_bytes()
    }

    /// wip
    pub fn end_path_bytes(&self) -> &[u8] {
        self.end_path.path.as_bytes()
    }

    /// wip
    pub fn start_block(&self) -> Option<usize> {
        self.start_path.block
    }

    /// wip
    pub fn end_block(&self) -> Option<usize> {
        self.end_path.block
    }

    /// wip
    pub fn hash_type(&self) -> &str {
        &self.hash_type
    }

    /// wip
    pub fn hash(&self) -> &[u8] {
        &self.hash
    }
}


impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::Io(ref err) => err.description(),
            ParseError::MissingKeyword(_) => "missing keyword in manifest",
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
            ParseError::ParseInt(ref e) => write!(fmt, "{}", e),
            ParseError::Utf8(ref e) => write!(fmt, "{}", e),
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

impl From<FromUtf8Error> for ParseError {
    fn from(err: FromUtf8Error) -> ParseError {
        ParseError::Utf8(err)
    }
}


impl<R: BufRead> ManifestParser<R> {
    pub fn new(input: R) -> Self {
        ManifestParser {
            input: input,
            buf: vec![],
        }
    }

    pub fn parse(mut self) -> Result<Manifest, ParseError> {
        let hostname = try!(self.read_param_str("Hostname"));
        let local_dir = RawPath::with_bytes(try!(self.read_param_bytes("Localdir")));

        let mut volumes = Vec::new();
        while let Some((vol, i)) = try!(self.read_volume()) {
            // resize volumes if necessary
            if i >= volumes.len() {
                volumes.reserve(i + 1);
                for _ in volumes.len()..i + 1 {
                    volumes.push(None);
                }
            }
            volumes[i] = Some(vol);
        }

        Ok(Manifest {
            hostname: hostname,
            local_dir: local_dir,
            volumes: volumes,
        })
    }

    fn read_volume(&mut self) -> Result<Option<(Volume, usize)>, ParseError> {
        // volume number
        let mut param = try!(self.read_param_str("Volume"));
        if param.ends_with(":") {
            param.pop();
        }
        let num = try!(usize::from_str(&param));
        let start_path = try!(self.read_path_block("StartingPath"));
        let end_path = try!(self.read_path_block("EndingPath"));
        let (htype, h) = try!(self.read_hash_param());

        let vol = Volume {
            start_path: start_path,
            end_path: end_path,
            hash_type: htype,
            hash: h,
        };
        Ok(Some((vol, num)))
    }

    fn read_path_block(&mut self, key: &str) -> Result<PathBlock, ParseError> {
        try!(self.consume_whitespace());
        if !try!(self.consume_keyword(key)) {
            return Err(ParseError::MissingKeyword(key.to_owned()));
        }
        try!(self.consume_whitespace());
        let path = try!(self.read_param_value());
        try!(self.consume_whitespace());
        let block = if !try!(self.consume_byte(b'\n')) {
            let bytes = try!(self.read_param_value());
            let s = try!(String::from_utf8(bytes));
            let num = try!(usize::from_str(&s));
            Some(num)
        } else {
            None
        };
        Ok(PathBlock {
            path: RawPath::with_bytes(path),
            block: block,
        })
    }

    fn read_hash_param(&mut self) -> Result<(String, Vec<u8>), ParseError> {
        try!(self.consume_whitespace());
        if !try!(self.consume_keyword("Hash")) {
            return Err(ParseError::MissingKeyword("Hash".to_owned()));
        }
        try!(self.consume_whitespace());
        let htype = try!(self.read_param_value_str());
        try!(self.consume_whitespace());
        let mut hash = try!(self.read_param_value());
        for b in &mut hash {
            *b -= b'0'
        }
        try!(self.consume_newline());

        Ok((htype, hash))
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

    fn read_param_value_str(&mut self) -> Result<String, ParseError> {
        let bytes = try!(self.read_param_value());
        String::from_utf8(bytes).map_err(From::from)
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
                    let num = ((self.buf[i + 2] - b'0') << 4) + self.buf[i + 3] - b'0';
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


#[inline]
fn match_keyword(buf: &[u8], key: &str) -> bool {
    str::from_utf8(&buf).ok().map_or(false, |s| s == key)
}


#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;

    #[test]
    fn parse_no_err() {
        let file = File::open("tests/manifest/full1.manifest").unwrap();
        let mut bfile = BufReader::new(file);
        Manifest::parse(&mut bfile).unwrap();
    }
}
