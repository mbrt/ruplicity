use regex::Regex;

pub struct Collection {
    pub backup_chain : Vec<BackupSet>
}

pub struct BackupSet {
    files : Vec<FileName>
}

impl Collection {
    pub fn open(path : &str) -> Self {
        Collection{ backup_chain : Vec::new() }
    }
}

enum FileType {
    FullSig,
    //NewSig,
    //Inc,
    //Full
}

struct FileName {
    ftype : FileType,
    // TODO enable those fields
    //manifest : bool,
    //volume_number : i32,
    //time : String,
    //start_time : String,
    //end_time : String,
    //compressed : bool,
    //encrypted : bool,
    //partial : bool
}

struct FileNameParser {
    full_vol_re_short : Regex
}

impl FileNameParser {
    pub fn new() -> Self {
        FileNameParser {
            full_vol_re_short : Regex::new(r"^duplicity-full\.(?P<time>.*?)\.vol(?P<num>[0-9]+)\.difftar(?P<partial>(\.part))?($|\.)").unwrap()
        }
    }

    pub fn parse(&self, filename : &str) -> FileName {
        use std::ascii::AsciiExt;

        let lower_fname = filename.to_ascii_lowercase();
        self.check_full(&lower_fname).unwrap()
    }

    fn check_full(&self, filename : &str) -> Option<FileName> {
        if let Some(captures) = self.full_vol_re_short.captures(filename) {
            return None;
        }
        return None;
    }
}


#[cfg(test)]
mod test {
    use super::FileNameParser;

    #[test]
    fn parser_test() {
        let parser = FileNameParser::new();
    }
}
