// TODO: Make this non public
pub mod file_naming;

use std::collections::HashMap;
use self::file_naming::{FileName, FileType, FileNameParser};


pub struct BackupSet {
    pub file_type : FileType,
    pub time : String,
    pub start_time : String,
    pub end_time : String,
    pub compressed : bool,
    pub encrypted : bool,
    pub partial : bool,
    pub manifest_path : String,
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
        }
        else {
            // check if the file is from the same backup set
            if self.file_type != pr.file_type ||
                self.time != pr.time ||
                self.start_time != pr.start_time ||
                self.end_time != pr.end_time {
                    return false;
            }
            else {
                // fix encrypted flag
                if self.encrypted != pr.encrypted &&
                    self.partial && pr.encrypted {
                        self.encrypted = pr.encrypted;
                    }
            }
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

    pub fn is_complete(&self) -> bool {
        !self.manifest_path.is_empty()
    }

    pub fn get_time(&self) -> &str {
        if self.time.is_empty() {
            self.end_time.as_ref()
        }
        else {
            self.time.as_ref()
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


pub struct BackupChain {
    fullset : BackupSet,
    incset_list : Vec<BackupSet>,
    start_time : String,
    end_time : String
}

impl BackupChain {
    /// Create a new BackupChain starting from a full backup set.
    pub fn new(fullset : BackupSet) -> Self {
        assert_eq!(fullset.file_type, FileType::Full);
        let time = fullset.time.clone();

        BackupChain{
            fullset : fullset,
            incset_list : Vec::new(),
            start_time : time.clone(),
            end_time : time
        }
    }

    /// Adds the given incremental backkup element to the backup chain if possible,
    /// returns it back otherwise.
    pub fn add_inc(&mut self, incset : BackupSet) -> Option<BackupSet> {
        if self.end_time == incset.start_time {
            self.end_time = incset.time.clone();
            self.incset_list.push(incset);
            None
        }
        else {
            // replace the last element if the end time comes before
            let replace_last = self.incset_list.last().map_or(false,
                |last| incset.start_time == last.start_time && incset.end_time > last.end_time);
            if replace_last {
                self.end_time = incset.time.clone();
                self.incset_list.pop();
                self.incset_list.push(incset);
                None
            }
            else {
                // ignore the given incremental backup set
                Some(incset)
            }
        }
    }
}


pub type FileNameList = Vec<String>;
type BackupSetList = Vec<BackupSet>;
type BackupChains = Vec<BackupChain>;

pub struct CollectionsStatus;

impl CollectionsStatus {
    pub fn compute_backup_chains(&mut self, filename_list : &FileNameList) {
        let mut sets = self.compute_backup_sets(filename_list);
        self.sort_backup_sets(&mut sets);
    }

    fn compute_backup_sets(&self, filename_list : &FileNameList) -> BackupSetList {
        let mut sets = BackupSetList::new();
        let parser = FileNameParser::new();
        for filename in filename_list.iter() {
            if let Some(filename_info) = parser.parse(filename.as_ref()) {
                let mut inserted = false;
                for set in sets.iter_mut() {
                    if set.add_filename(filename.as_ref(), &filename_info) {
                        inserted = true;
                        break;
                    }
                }
                if !inserted {
                    let mut new_set = BackupSet::new();
                    new_set.add_filename(filename.as_ref(), &filename_info);
                    sets.push(new_set);
                }
            }
        }
        sets
    }

    fn sort_backup_sets(&self, set_list : &mut BackupSetList) {
        set_list.sort_by(|a, b| a.get_time().cmp(b.get_time()));
    }

    fn add_to_chains(&self, set_list : &BackupSetList) {
        let chains = BackupChains::new();
        for set in set_list.iter() {
        }
    }
}


#[cfg(test)]
mod test {
    use super::BackupSet;
    use super::file_naming::{FileType, FileNameParser};

    #[test]
    fn parse_and_add() {
        let full1_name = "duplicity-full.20150617T182545Z.vol1.difftar.gz";
        let manifest1_name = "duplicity-full.20150617T182545Z.manifest";
        let inc1_name = "duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz";

        let parser = FileNameParser::new();
        let full1 = parser.parse(full1_name).unwrap();
        let manifest1 = parser.parse(manifest1_name).unwrap();
        let inc1 = parser.parse(inc1_name).unwrap();

        let mut set = BackupSet::new();
        // add to set
        assert!(set.add_filename(full1_name, &full1));
        assert!(set.add_filename(manifest1_name, &manifest1));
        assert!(!set.add_filename(inc1_name, &inc1));
        // test results
        assert_eq!(set.file_type, FileType::Full);
        assert_eq!(set.time, "20150617t182545z");
        assert_eq!(set.start_time, "");
        assert_eq!(set.end_time, "");
        assert!(set.compressed);
        assert!(!set.encrypted);
        assert!(!set.partial);
        assert_eq!(set.manifest_path, manifest1_name);
    }
}
