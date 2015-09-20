use regex::Regex;


#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum FileType {
    FullSig,
    NewSig,
    Inc,
    Full
}

#[derive(Eq, PartialEq, Debug)]
pub struct FileName {
    pub file_type : FileType,
    pub manifest : bool,
    pub volume_number : i32,
    pub time : String,
    pub start_time : String,
    pub end_time : String,
    pub compressed : bool,
    pub encrypted : bool,
    pub partial : bool
}

impl FileName {
    /// Builder pattern for FileName
    pub fn new() -> Self {
        FileName{
            file_type : FileType::Full,
            manifest : false,
            volume_number : 0,
            time : String::new(),
            start_time : String::new(),
            end_time : String::new(),
            compressed : false,
            encrypted : false,
            partial : false}
    }
}

gen_setters!(FileName,
    file_type : FileType,
    manifest : bool,
    volume_number : i32,
    time : String,
    start_time : String,
    end_time : String,
    compressed : bool,
    encrypted : bool,
    partial : bool
);


pub struct FileNameInfo<'a> {
    pub file_name : &'a str,
    pub info : FileName
}

impl<'a> FileNameInfo<'a> {
    pub fn new(name : &'a str, info : FileName) -> Self {
        FileNameInfo {
            file_name : &name,
            info : info
        }
    }
}


pub struct FileNameParser {
    full_vol_re : Regex,
    full_manifest_re : Regex,
    inc_vol_re : Regex,
    inc_manifest_re : Regex,
    full_sig_re : Regex,
    new_sig_re : Regex
}

impl FileNameParser {
    pub fn new() -> Self {
        FileNameParser {
            full_vol_re : Regex::new(r"^duplicity-full\.(?P<time>.*?)\.vol(?P<num>[0-9]+)\.difftar(?P<partial>(\.part))?($|\.)").unwrap(),
            full_manifest_re : Regex::new(r"^duplicity-full\.(?P<time>.*?)\.manifest(?P<partial>(\.part))?($|\.)").unwrap(),
            inc_vol_re : Regex::new(r"^duplicity-inc\.(?P<start_time>.*?)\.to\.(?P<end_time>.*?)\.vol(?P<num>[0-9]+)\.difftar($|\.)").unwrap(),
            inc_manifest_re : Regex::new(r"^duplicity-inc\.(?P<start_time>.*?)\.to\.(?P<end_time>.*?)\.manifest(?P<partial>(\.part))?(\.|$)").unwrap(),
            full_sig_re : Regex::new(r"^duplicity-full-signatures\.(?P<time>.*?)\.sigtar(?P<partial>(\.part))?(\.|$)").unwrap(),
            new_sig_re : Regex::new(r"^duplicity-new-signatures\.(?P<start_time>.*?)\.to\.(?P<end_time>.*?)\.sigtar(?P<partial>(\.part))?(\.|$)").unwrap(),
        }
    }

    pub fn parse(&self, filename : &str) -> Option<FileName> {
        use std::ascii::AsciiExt;

        let lower_fname = filename.to_ascii_lowercase();
        let mut opt_result = self.check_full(&lower_fname)
            .or(self.check_inc(&lower_fname))
            .or(self.check_sig(&lower_fname));

        // write encrypted and compressed properties
        // independently of which type of file is
        if let Some(ref mut result) = opt_result {
            result.compressed = self.is_compressed(lower_fname.as_ref());
            result.encrypted = self.is_encrypted(lower_fname.as_ref());
        }
        opt_result
    }

    fn check_full(&self, filename : &str) -> Option<FileName> {
        if let Some(captures) = self.full_vol_re.captures(filename) {
            let time = captures.name("time").unwrap();
            // TODO: str2time
            let vol_num = try_opt!(self.get_vol_num(captures.name("num").unwrap()));
            return Some(FileName::new().file_type(FileType::Full)
                        .volume_number(vol_num)
                        .time(time.to_owned()));
        }
        if let Some(captures) = self.full_manifest_re.captures(filename) {
            let time = captures.name("time").unwrap();
            // TODO: str2time
            return Some(FileName::new().file_type(FileType::Full)
                        .manifest(true)
                        .time(time.to_owned())
                        .partial(captures.name("partial").is_some()));
        }
        return None;
    }

    fn check_inc(&self, filename : &str) -> Option<FileName> {
        if let Some(captures) = self.inc_vol_re.captures(filename) {
            let start_time = captures.name("start_time").unwrap();
            let end_time = captures.name("end_time").unwrap();
            // TODO: str2time
            let vol_num = try_opt!(self.get_vol_num(captures.name("num").unwrap()));
            return Some(FileName::new().file_type(FileType::Inc)
                        .start_time(start_time.to_owned())
                        .end_time(end_time.to_owned())
                        .volume_number(vol_num));
        }
        if let Some(captures) = self.inc_manifest_re.captures(filename) {
            let start_time = captures.name("start_time").unwrap();
            let end_time = captures.name("end_time").unwrap();
            // TODO: str2time
            return Some(FileName::new().file_type(FileType::Inc)
                        .start_time(start_time.to_owned())
                        .end_time(end_time.to_owned())
                        .manifest(true)
                        .partial(captures.name("partial").is_some()));
        }
        return None;
    }

    fn check_sig(&self, filename : &str) -> Option<FileName> {
        if let Some(captures) = self.full_sig_re.captures(filename) {
            let time = captures.name("time").unwrap();
            // TODO: str2time
            return Some(FileName::new().file_type(FileType::FullSig)
                        .time(time.to_owned())
                        .partial(captures.name("partial").is_some()));
        }
        if let Some(captures) = self.new_sig_re.captures(filename) {
            let start_time = captures.name("start_time").unwrap();
            let end_time = captures.name("end_time").unwrap();
            // TODO: str2time
            return Some(FileName::new().file_type(FileType::NewSig)
                        .start_time(start_time.to_owned())
                        .end_time(end_time.to_owned())
                        .partial(captures.name("partial").is_some()));
        }
        return None;
    }

    fn get_vol_num(&self, s : &str) -> Option<i32> {
        s.parse::<i32>().ok()
    }

    fn is_encrypted(&self, s : &str) -> bool {
        s.ends_with(".gpg") || s.ends_with(".g")
    }

    fn is_compressed(&self, s : &str) -> bool {
        s.ends_with(".gz") || s.ends_with(".z")
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parser_test() {
        let parser = FileNameParser::new();
        // invalid
        assert_eq!(parser.parse("invalid"), None);
        // full
        assert_eq!(parser.parse("duplicity-full.20150617T182545Z.vol1.difftar.gz"),
                   Some(FileName{file_type : FileType::Full,
                                 manifest : false,
                                 volume_number : 1,
                                 time : "20150617t182545z".to_owned(),
                                 start_time : String::new(),
                                 end_time : String::new(),
                                 compressed : true,
                                 encrypted: false,
                                 partial : false}));
        assert_eq!(parser.parse("duplicity-full.20150617T182545Z.manifest"),
                   Some(FileName{file_type : FileType::Full,
                                 manifest : true,
                                 volume_number : 0,
                                 time : "20150617t182545z".to_owned(),
                                 start_time : String::new(),
                                 end_time : String::new(),
                                 compressed : false,
                                 encrypted: false,
                                 partial : false}));
        // inc
        assert_eq!(parser.parse("duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz"),
                   Some(FileName{file_type : FileType::Inc,
                                 manifest : false,
                                 volume_number : 1,
                                 time : String::new(),
                                 start_time : "20150617t182629z".to_owned(),
                                 end_time : "20150617t182650z".to_owned(),
                                 compressed : true,
                                 encrypted: false,
                                 partial : false}));
        assert_eq!(parser.parse("duplicity-inc.20150617T182545Z.to.20150617T182629Z.manifest"),
                   Some(FileName{file_type : FileType::Inc,
                                 manifest : true,
                                 volume_number : 0,
                                 time : String::new(),
                                 start_time : "20150617t182545z".to_owned(),
                                 end_time : "20150617t182629z".to_owned(),
                                 compressed : false,
                                 encrypted: false,
                                 partial : false}));
        // new sig
        assert_eq!(parser.parse("duplicity-new-signatures.20150617T182545Z.to.20150617T182629Z.sigtar.gz"),
                   Some(FileName{file_type : FileType::NewSig,
                                 manifest : false,
                                 volume_number : 0,
                                 time : String::new(),
                                 start_time : "20150617t182545z".to_owned(),
                                 end_time : "20150617t182629z".to_owned(),
                                 compressed : true,
                                 encrypted: false,
                                 partial : false}));
        // full sig
        assert_eq!(parser.parse("duplicity-full-signatures.20150617T182545Z.sigtar.gz"),
                   Some(FileName{file_type : FileType::FullSig,
                                 manifest : false,
                                 volume_number : 0,
                                 time : "20150617t182545z".to_owned(),
                                 start_time : String::new(),
                                 end_time : String::new(),
                                 compressed : true,
                                 encrypted: false,
                                 partial : false}));
    }

    #[test]
    fn time_test() {
        let result = ::time::strptime("20150617t182545Z", "%Y%m%dt%H%M%S%Z").unwrap();
        println!("{}", ::time::strftime("%a %d/%m/%Y %H:%M:%S", &result).unwrap());
    }
}
