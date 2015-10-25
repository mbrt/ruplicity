use chrono::DateTime;
use regex::Regex;

use time_utils::{self, parse_time_str, Timestamp};


#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum FileType {
    FullSig,
    NewSig,
    Inc,
    Full
}

#[derive(Eq, PartialEq, Debug)]
pub struct FileName {
    pub file_type: FileType,
    pub manifest: bool,
    pub volume_number: i32,
    pub time: Timestamp,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
    pub compressed: bool,
    pub encrypted: bool,
    pub partial: bool
}

pub struct FileNameInfo<'a> {
    pub file_name: &'a str,
    pub info: FileName
}

pub struct FileNameParser {
    full_vol_re: Regex,
    full_manifest_re: Regex,
    inc_vol_re: Regex,
    inc_manifest_re: Regex,
    full_sig_re: Regex,
    new_sig_re: Regex
}


impl<'a> FileNameInfo<'a> {
    pub fn new(name: &'a str, info: FileName) -> Self {
        FileNameInfo {
            file_name: &name,
            info: info
        }
    }
}


impl FileName {
    /// Builder pattern for FileName
    pub fn new() -> Self {
        FileName{
            file_type: FileType::Full,
            manifest: false,
            volume_number: 0,
            time: time_utils::DEFAULT_TIMESPEC,
            start_time: time_utils::DEFAULT_TIMESPEC,
            end_time: time_utils::DEFAULT_TIMESPEC,
            compressed: false,
            encrypted: false,
            partial: false}
    }
}

gen_setters!(FileName,
    file_type: FileType,
    manifest: bool,
    volume_number: i32,
    time: Timestamp,
    start_time: Timestamp,
    end_time: Timestamp,
    // not used for now: enable if needed
    //compressed: bool,
    //encrypted: bool,
    partial: bool
);


impl FileNameParser {
    pub fn new() -> Self {
        FileNameParser {
            full_vol_re: Regex::new(r"^duplicity-full\.(?P<time>.*?)\.vol(?P<num>[0-9]+)\.difftar(?P<partial>(\.part))?($|\.)").unwrap(),
            full_manifest_re: Regex::new(r"^duplicity-full\.(?P<time>.*?)\.manifest(?P<partial>(\.part))?($|\.)").unwrap(),
            inc_vol_re: Regex::new(r"^duplicity-inc\.(?P<start_time>.*?)\.to\.(?P<end_time>.*?)\.vol(?P<num>[0-9]+)\.difftar($|\.)").unwrap(),
            inc_manifest_re: Regex::new(r"^duplicity-inc\.(?P<start_time>.*?)\.to\.(?P<end_time>.*?)\.manifest(?P<partial>(\.part))?(\.|$)").unwrap(),
            full_sig_re: Regex::new(r"^duplicity-full-signatures\.(?P<time>.*?)\.sigtar(?P<partial>(\.part))?(\.|$)").unwrap(),
            new_sig_re: Regex::new(r"^duplicity-new-signatures\.(?P<start_time>.*?)\.to\.(?P<end_time>.*?)\.sigtar(?P<partial>(\.part))?(\.|$)").unwrap(),
        }
    }

    pub fn parse(&self, filename: &str) -> Option<FileName> {
        use std::ascii::AsciiExt;

        let lower_fname = filename.to_ascii_lowercase();
        let mut opt_result = self.check_full(&lower_fname)
            .or(self.check_inc(&lower_fname))
            .or(self.check_sig(&lower_fname));

        // write encrypted and compressed properties
        // independently from the file type
        if let Some(ref mut result) = opt_result {
            result.compressed = self.is_compressed(lower_fname.as_ref());
            result.encrypted = self.is_encrypted(lower_fname.as_ref());
        }
        opt_result
    }

    fn check_full(&self, filename: &str) -> Option<FileName> {
        if let Some(captures) = self.full_vol_re.captures(filename) {
            let time = try_opt!(parse_time_str(captures.name("time").unwrap()));
            let vol_num = try_opt!(self.get_vol_num(captures.name("num").unwrap()));
            return Some(FileName::new().file_type(FileType::Full)
                        .volume_number(vol_num)
                        .time(time));
        }
        if let Some(captures) = self.full_manifest_re.captures(filename) {
            let time = try_opt!(parse_time_str(captures.name("time").unwrap()));
            return Some(FileName::new().file_type(FileType::Full)
                        .manifest(true)
                        .time(time)
                        .partial(captures.name("partial").is_some()));
        }
        None
    }

    fn check_inc(&self, filename: &str) -> Option<FileName> {
        if let Some(captures) = self.inc_vol_re.captures(filename) {
            let start_time = try_opt!(parse_time_str(captures.name("start_time").unwrap()));
            let end_time = try_opt!(parse_time_str(captures.name("end_time").unwrap()));
            let vol_num = try_opt!(self.get_vol_num(captures.name("num").unwrap()));
            return Some(FileName::new().file_type(FileType::Inc)
                        .start_time(start_time)
                        .end_time(end_time)
                        .volume_number(vol_num));
        }
        if let Some(captures) = self.inc_manifest_re.captures(filename) {
            let start_time = try_opt!(parse_time_str(captures.name("start_time").unwrap()));
            let end_time = try_opt!(parse_time_str(captures.name("end_time").unwrap()));
            return Some(FileName::new().file_type(FileType::Inc)
                        .start_time(start_time)
                        .end_time(end_time)
                        .manifest(true)
                        .partial(captures.name("partial").is_some()));
        }
        None
    }

    fn check_sig(&self, filename: &str) -> Option<FileName> {
        if let Some(captures) = self.full_sig_re.captures(filename) {
            let time = try_opt!(parse_time_str(captures.name("time").unwrap()));
            return Some(FileName::new().file_type(FileType::FullSig)
                        .time(time)
                        .partial(captures.name("partial").is_some()));
        }
        if let Some(captures) = self.new_sig_re.captures(filename) {
            let start_time = try_opt!(parse_time_str(captures.name("start_time").unwrap()));
            let end_time = try_opt!(parse_time_str(captures.name("end_time").unwrap()));
            return Some(FileName::new().file_type(FileType::NewSig)
                        .start_time(start_time)
                        .end_time(end_time)
                        .partial(captures.name("partial").is_some()));
        }
        None
    }

    fn get_vol_num(&self, s: &str) -> Option<i32> {
        s.parse::<i32>().ok()
    }

    fn is_encrypted(&self, s: &str) -> bool {
        s.ends_with(".gpg") || s.ends_with(".g")
    }

    fn is_compressed(&self, s: &str) -> bool {
        s.ends_with(".gz") || s.ends_with(".z")
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use time_utils::{parse_time_str, DEFAULT_TIMESPEC};

    #[test]
    fn parser_test() {
        let parser = FileNameParser::new();
        // invalid
        assert_eq!(parser.parse("invalid"), None);
        // full
        assert_eq!(parser.parse("duplicity-full.20150617T182545Z.vol1.difftar.gz"),
                   Some(FileName{file_type: FileType::Full,
                                 manifest: false,
                                 volume_number: 1,
                                 time: parse_time_str("20150617t182545z").unwrap(),
                                 start_time: DEFAULT_TIMESPEC,
                                 end_time: DEFAULT_TIMESPEC,
                                 compressed: true,
                                 encrypted: false,
                                 partial: false}));
        assert_eq!(parser.parse("duplicity-full.20150617T182545Z.manifest"),
                   Some(FileName{file_type: FileType::Full,
                                 manifest: true,
                                 volume_number: 0,
                                 time: parse_time_str("20150617t182545z").unwrap(),
                                 start_time: DEFAULT_TIMESPEC,
                                 end_time: DEFAULT_TIMESPEC,
                                 compressed: false,
                                 encrypted: false,
                                 partial: false}));
        // inc
        assert_eq!(parser.parse("duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz"),
                   Some(FileName{file_type: FileType::Inc,
                                 manifest: false,
                                 volume_number: 1,
                                 time: DEFAULT_TIMESPEC,
                                 start_time: parse_time_str("20150617t182629z").unwrap(),
                                 end_time: parse_time_str("20150617t182650z").unwrap(),
                                 compressed: true,
                                 encrypted: false,
                                 partial: false}));
        assert_eq!(parser.parse("duplicity-inc.20150617T182545Z.to.20150617T182629Z.manifest"),
                   Some(FileName{file_type: FileType::Inc,
                                 manifest: true,
                                 volume_number: 0,
                                 time: DEFAULT_TIMESPEC,
                                 start_time: parse_time_str("20150617t182545z").unwrap(),
                                 end_time: parse_time_str("20150617t182629z").unwrap(),
                                 compressed: false,
                                 encrypted: false,
                                 partial: false}));
        // new sig
        assert_eq!(parser.parse("duplicity-new-signatures.20150617T182545Z.to.20150617T182629Z.sigtar.gz"),
                   Some(FileName{file_type: FileType::NewSig,
                                 manifest: false,
                                 volume_number: 0,
                                 time: DEFAULT_TIMESPEC,
                                 start_time: parse_time_str("20150617t182545z").unwrap(),
                                 end_time: parse_time_str("20150617t182629z").unwrap(),
                                 compressed: true,
                                 encrypted: false,
                                 partial: false}));
        // full sig
        assert_eq!(parser.parse("duplicity-full-signatures.20150617T182545Z.sigtar.gz"),
                   Some(FileName{file_type: FileType::FullSig,
                                 manifest: false,
                                 volume_number: 0,
                                 time: parse_time_str("20150617t182545z").unwrap(),
                                 start_time: DEFAULT_TIMESPEC,
                                 end_time: DEFAULT_TIMESPEC,
                                 compressed: true,
                                 encrypted: false,
                                 partial: false}));
    }

    #[test]
    fn time_test() {
        use time::{strptime, strftime, at_utc, Tm};

        // parse
        let tm = strptime("20150617t182545Z", "%Y%m%dt%H%M%S%Z").unwrap();
        // format
        assert_eq!(strftime("%a %d/%m/%Y %H:%M:%S", &tm).unwrap(), "Sun 17/06/2015 18:25:45");
        assert_eq!(format!("{}", tm.rfc3339()), "2015-06-17T18:25:45Z");
        // store in Timestamp and restore in Tm
        let ts = tm.to_timespec();
        let tm1 = at_utc(ts);
        // somehow they don't have the same identical structure :(
        // assert_eq!(tm, tm1);
        // test equally formatted
        let format_fn = |tm: &Tm| { format!("{}", tm.rfc3339()) };
        assert_eq!(format_fn(&tm), format_fn(&tm1));
    }
}
