use regex::Regex;
use time::Timespec;

use timefmt::parse_time_str;


pub struct FileNameInfo<'a> {
    pub file_name: &'a str,
    pub info: Info,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Info {
    pub tp: Type,
    pub compressed: bool,
    pub encrypted: bool,
}

#[derive(Eq, PartialEq, Debug)]
pub enum Type {
    Full {
        time: Timespec,
        volume_number: usize,
    },
    FullManifest {
        time: Timespec,
        partial: bool,
    },
    Inc {
        start_time: Timespec,
        end_time: Timespec,
        volume_number: usize,
    },
    IncManifest {
        start_time: Timespec,
        end_time: Timespec,
        partial: bool,
    },
    FullSig {
        time: Timespec,
        partial: bool,
    },
    NewSig {
        start_time: Timespec,
        end_time: Timespec,
        partial: bool,
    },
}

pub struct FileNameParser {
    full_vol_re: Regex,
    full_manifest_re: Regex,
    inc_vol_re: Regex,
    inc_manifest_re: Regex,
    full_sig_re: Regex,
    new_sig_re: Regex,
}


impl<'a> FileNameInfo<'a> {
    pub fn new(name: &'a str, info: Info) -> Self {
        FileNameInfo {
            file_name: &name,
            info: info,
        }
    }

    pub fn start_time(&self) -> Timespec {
        self.info.tp.time_range().0
    }
}

impl Type {
    pub fn time_range(&self) -> (Timespec, Timespec) {
        match *self {
            Type::Full{ time, .. } |
            Type::FullSig{ time, .. } |
            Type::FullManifest{ time, .. } => (time, time),
            Type::Inc{ start_time, end_time, .. } |
            Type::IncManifest{ start_time, end_time, .. } |
            Type::NewSig{ start_time, end_time, .. } => (start_time, end_time),
        }
    }
}


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

    pub fn parse(&self, filename: &str) -> Option<Info> {
        use std::ascii::AsciiExt;

        let lower_fname = filename.to_ascii_lowercase();
        let opt_type = self.check_full(&lower_fname)
                           .or(self.check_inc(&lower_fname))
                           .or(self.check_sig(&lower_fname));
        opt_type.map(|t| {
            Info {
                tp: t,
                compressed: is_compressed(lower_fname.as_ref()),
                encrypted: is_encrypted(lower_fname.as_ref()),
            }
        })
    }

    fn check_full(&self, filename: &str) -> Option<Type> {
        if let Some(captures) = self.full_vol_re.captures(filename) {
            let time = try_opt!(parse_time_str(captures.name("time").unwrap()));
            let vol_num = try_opt!(get_vol_num(captures.name("num").unwrap()));
            Some(Type::Full {
                time: time,
                volume_number: vol_num,
            })
        } else if let Some(captures) = self.full_manifest_re.captures(filename) {
            let time = try_opt!(parse_time_str(captures.name("time").unwrap()));
            Some(Type::FullManifest {
                time: time,
                partial: captures.name("partial").is_some(),
            })
        } else {
            None
        }
    }

    fn check_inc(&self, filename: &str) -> Option<Type> {
        if let Some(captures) = self.inc_vol_re.captures(filename) {
            let start_time = try_opt!(parse_time_str(captures.name("start_time").unwrap()));
            let end_time = try_opt!(parse_time_str(captures.name("end_time").unwrap()));
            let vol_num = try_opt!(get_vol_num(captures.name("num").unwrap()));
            Some(Type::Inc {
                start_time: start_time,
                end_time: end_time,
                volume_number: vol_num,
            })
        } else if let Some(captures) = self.inc_manifest_re.captures(filename) {
            let start_time = try_opt!(parse_time_str(captures.name("start_time").unwrap()));
            let end_time = try_opt!(parse_time_str(captures.name("end_time").unwrap()));
            Some(Type::IncManifest {
                start_time: start_time,
                end_time: end_time,
                partial: captures.name("partial").is_some(),
            })
        } else {
            None
        }
    }

    fn check_sig(&self, filename: &str) -> Option<Type> {
        if let Some(captures) = self.full_sig_re.captures(filename) {
            let time = try_opt!(parse_time_str(captures.name("time").unwrap()));
            Some(Type::FullSig {
                time: time,
                partial: captures.name("partial").is_some(),
            })
        } else if let Some(captures) = self.new_sig_re.captures(filename) {
            let start_time = try_opt!(parse_time_str(captures.name("start_time").unwrap()));
            let end_time = try_opt!(parse_time_str(captures.name("end_time").unwrap()));
            Some(Type::NewSig {
                start_time: start_time,
                end_time: end_time,
                partial: captures.name("partial").is_some(),
            })
        } else {
            None
        }
    }
}


fn get_vol_num(s: &str) -> Option<usize> {
    s.parse::<usize>().ok()
}

fn is_encrypted(s: &str) -> bool {
    s.ends_with(".gpg") || s.ends_with(".g")
}

fn is_compressed(s: &str) -> bool {
    s.ends_with(".gz") || s.ends_with(".z")
}


#[cfg(test)]
mod test {
    use super::*;
    use timefmt::parse_time_str;

    #[test]
    fn parser_test() {
        let parser = FileNameParser::new();
        // invalid
        assert_eq!(parser.parse("invalid"), None);
        // full
        assert_eq!(parser.parse("duplicity-full.20150617T182545Z.vol1.difftar.gz"),
                   Some(Info {
                       tp: Type::Full {
                           time: parse_time_str("20150617t182545z").unwrap(),
                           volume_number: 1,
                       },
                       compressed: true,
                       encrypted: false,
                   }));
        // full manifest
        assert_eq!(parser.parse("duplicity-full.20150617T182545Z.manifest"),
                   Some(Info {
                       tp: Type::FullManifest {
                           time: parse_time_str("20150617t182545z").unwrap(),
                           partial: false,
                       },
                       compressed: false,
                       encrypted: false,
                   }));
        // inc
        assert_eq!(parser.parse("duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz"),
                   Some(Info{ tp: Type::Inc{ start_time: parse_time_str("20150617t182629z").unwrap(),
                                             end_time: parse_time_str("20150617t182650z").unwrap(),
                                             volume_number: 1 },
                              compressed: true,
                              encrypted: false }));
        // inc manifest
        assert_eq!(parser.parse("duplicity-inc.20150617T182545Z.to.20150617T182629Z.manifest"),
                   Some(Info {
                       tp: Type::IncManifest {
                           start_time: parse_time_str("20150617t182545z").unwrap(),
                           end_time: parse_time_str("20150617t182629z").unwrap(),
                           partial: false,
                       },
                       compressed: false,
                       encrypted: false,
                   }));
        // new sig
        assert_eq!(parser.parse("duplicity-new-signatures.20150617T182545Z.to.20150617T182629Z.sigtar.gz"),
                   Some(Info{ tp: Type::NewSig{ start_time: parse_time_str("20150617t182545z").unwrap(),
                                                end_time: parse_time_str("20150617t182629z").unwrap(),
                                                partial: false },
                              compressed: true,
                              encrypted: false }));
        // full sig
        assert_eq!(parser.parse("duplicity-full-signatures.20150617T182545Z.sigtar.gz"),
                   Some(Info {
                       tp: Type::FullSig {
                           time: parse_time_str("20150617t182545z").unwrap(),
                           partial: false,
                       },
                       compressed: true,
                       encrypted: false,
                   }));
    }
}
