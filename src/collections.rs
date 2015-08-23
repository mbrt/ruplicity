use regex::Regex;

enum FileType {
    FullSig,
    NewSig,
    Inc,
    Full
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

static FILE_PREFIX : &'static str = "duplicity-";
//static full_vol_re_short : Regex = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();

impl FileName {
    pub fn parse(filename : &str) -> Self {
        use std::ascii::AsciiExt;

        let lower_fname = filename.to_ascii_lowercase();
        Self::check_full(&lower_fname).unwrap()
    }

    fn check_full(filename : &str) -> Option<Self> {
        None
    }
}
