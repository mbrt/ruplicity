// TODO: Make this non public
pub mod file_naming;

use std::collections::HashMap;
use self::file_naming::{FileName, FileType};


pub struct BackupSet {
    pub file_type : FileType,
    pub time : String,
    pub start_time : String,
    pub end_time : String,
    pub compressed : bool,
    pub encrypted : bool,
    pub partial : bool,
    manifest_path : String,
    volumes_paths : HashMap<i32, String>,
    info_set : bool,    // true if informations are set
}

impl BackupSet {
    pub fn new() -> Self {
        BackupSet{
            file_type : FileType::Full,
            time : String::new(),
            start_time : String::new(),
            end_time : String::new(),
            compressed : false,
            encrypted : false,
            partial : false,
            manifest_path : String::new(),
            volumes_paths : HashMap::new(),
            info_set : false
        }
    }

    /// Add a filename to given set. Return true if it fits.
    ///
    /// The filename will match the given set if it has the right
    /// times and is of the right type. The information will be set
    /// from the first filename given.
    pub fn add_filename(&mut self, fname : &str, pr : &FileName) -> bool {
        if !self.info_set {
            self.set_info(pr);
            true
        }
        else {
            // check if the file is from the same backup set
            if self.file_type != pr.file_type ||
                self.time != pr.time ||
                self.start_time != pr.start_time ||
                self.end_time != pr.end_time {
                    false
            }
            else {
                // fix encrypted flag
                if self.encrypted != pr.encrypted &&
                    self.partial && pr.encrypted {
                        self.encrypted = pr.encrypted;
                    }
                // set manifest or volume number
                if pr.manifest {
                    self.manifest_path = fname.to_owned();
                }
                else {
                    self.volumes_paths.insert(pr.volume_number, fname.to_owned());
                }
                true
            }
        }
    }

    fn set_info(&mut self, fname : &FileName) {
        self.file_type = fname.file_type;
        self.time = fname.time.clone();
        self.start_time = fname.start_time.clone();
        self.end_time = fname.end_time.clone();
        self.compressed = fname.compressed;
        self.encrypted = fname.encrypted;
        self.partial = fname.partial;
        self.info_set = true;
    }
}
