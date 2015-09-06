// TODO: Make this non public
pub mod file_naming;

use self::file_naming::FileName;

pub struct BackupSet {
    info_set : bool,    // true if informations are set
}

impl BackupSet {
    pub fn add_filename(&mut self, fname : &FileName) -> bool {
        true
    }
}
