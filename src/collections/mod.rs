// TODO: Make this non public
pub mod file_naming;

use std::collections::HashMap;
use std::fmt::{Display, Error, Formatter};
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

impl Display for BackupSet {
    fn fmt(&self, f : &mut Formatter) -> Result<(), Error> {
        match self.file_type {
            FileType::Full => {
                try!(write!(f, "Full, time: {}", self.time));
            },
            FileType::Inc => {
                try!(write!(f, "Incremental, start time: {}, end time: {}", self.start_time, self.end_time));
            },
            _ => { }
        }
        if self.compressed {
            try!(write!(f, ", compressed"));
        }
        if self.encrypted {
            try!(write!(f, ", encrypted"));
        }
        if self.partial {
            try!(write!(f, ", partial"));
        }
        if !self.manifest_path.is_empty() {
            try!(write!(f, "\n manifest: {}", self.manifest_path));
        }
        if !self.volumes_paths.is_empty() {
            try!(write!(f, "\n volumes:\n"));
            for (_, vol) in self.volumes_paths.iter() {
                try!(write!(f, " {}", vol));
            }
        }
        Ok(())
    }
}


pub struct BackupChain {
    pub fullset : BackupSet,
    pub incset_list : Vec<BackupSet>,
    pub start_time : String,
    pub end_time : String
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

    /// Adds the given incremental backup element to the backup chain if possible,
    /// returns it back otherwise.
    pub fn add_inc(&mut self, incset : BackupSet) -> Option<BackupSet> {
        if self.end_time == incset.start_time {
            self.end_time = incset.end_time.clone();
            self.incset_list.push(incset);
            None
        }
        else {
            // replace the last element if the end time comes before
            let replace_last = self.incset_list.last().map_or(false,
                |last| incset.start_time == last.start_time && incset.end_time > last.end_time);
            if replace_last {
                self.end_time = incset.end_time.clone();
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

impl Display for BackupChain {
    fn fmt(&self, f : &mut Formatter) -> Result<(), Error> {
        try!(write!(f, "start time: {}, end time: {}\n{}",
                    &self.start_time, &self.end_time, &self.fullset));
        for inc in self.incset_list.iter() {
            try!(write!(f, "\n{}", inc));
        }
        Ok(())
    }
}


pub struct SignatureChain {
    pub fullsig : String,
    pub inclist : Vec<String>,
    pub start_time : String,
    pub end_time : String
}

impl SignatureChain {
    /// Create a new SignatureChain starting from a full signature.
    pub fn new(fname : &str, pr : &FileName) -> Self {
        SignatureChain {
            fullsig : fname.to_owned(),
            inclist : Vec::new(),
            start_time : pr.time.clone(),
            end_time : pr.time.clone()
        }
    }

    /// Adds the given incremental signature to the signature chain if possible,
    /// returns false otherwise.
    pub fn add_new_sig(&mut self, fname : &str, pr : &FileName) -> bool {
        if pr.file_type != FileType::NewSig {
            false
        }
        else {
            self.inclist.push(fname.to_owned());
            self.end_time = pr.end_time.clone();
            true
        }
    }
}


pub type FileNameList<'a> = Vec<&'a str>;
pub type BackupChains = Vec<BackupChain>;
pub type SignatureChains = Vec<SignatureChain>;
type BackupSetList = Vec<BackupSet>;


pub struct CollectionsStatus {
    pub backup_chains : BackupChains,
    pub sig_chains : SignatureChains
}

impl CollectionsStatus {
    pub fn new() -> Self {
        CollectionsStatus {
            backup_chains : Vec::new(),
            sig_chains : Vec::new()
        }
    }

    pub fn from_filename_list(filename_list : &FileNameList) -> Self {
        let mut result = Self::new();
        result.compute_backup_chains(filename_list);
        result
    }

    fn compute_backup_chains(&mut self, filename_list : &FileNameList) {
        let mut sets = self.compute_backup_sets(filename_list);
        self.sort_backup_sets(&mut sets);
        self.add_to_backup_chains(sets);
        self.sort_chains();
    }

    fn compute_backup_sets(&self, filename_list : &FileNameList) -> BackupSetList {
        let mut sets = BackupSetList::new();
        let parser = FileNameParser::new();
        for filename in filename_list.iter() {
            if let Some(filename_info) = parser.parse(filename) {
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

    fn add_to_backup_chains(&mut self, set_list : BackupSetList) {
        for set in set_list.into_iter() {
            match set.file_type {
                FileType::Full => {
                    let new_chain = BackupChain::new(set);
                    self.backup_chains.push(new_chain);
                }
                FileType::Inc => {
                    let mut rejected_set = Some(set);
                    for chain in self.backup_chains.iter_mut() {
                        rejected_set = chain.add_inc(rejected_set.unwrap());
                        if rejected_set.is_none() {
                            break;
                        }
                    }
                    if let Some(_) = rejected_set {
                        // TODO: add to orphaned sets
                    }
                }
                _ => { continue; }
            }
        }
    }

    fn sort_chains(&mut self) {
        self.backup_chains.sort_by(|a, b| a.end_time.cmp(&b.end_time));
    }
}

impl Display for CollectionsStatus {
    fn fmt(&self, f : &mut Formatter) -> Result<(), Error> {
        for backup_chain in &self.backup_chains {
            try!(backup_chain.fmt(f));
        }
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use super::{BackupSet, CollectionsStatus};
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

    #[test]
    fn collection_status() {
        let filename_list = vec![
            "duplicity-full.20150617T182545Z.manifest",
            "duplicity-full.20150617T182545Z.vol1.difftar.gz",
            "duplicity-full-signatures.20150617T182545Z.sigtar.gz",
            "duplicity-inc.20150617T182545Z.to.20150617T182629Z.manifest",
            "duplicity-inc.20150617T182545Z.to.20150617T182629Z.vol1.difftar.gz",
            "duplicity-inc.20150617T182629Z.to.20150617T182650Z.manifest",
            "duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz",
            "duplicity-new-signatures.20150617T182545Z.to.20150617T182629Z.sigtar.gz",
            "duplicity-new-signatures.20150617T182629Z.to.20150617T182650Z.sigtar.gz"
        ];
        let collection_status = CollectionsStatus::from_filename_list(&filename_list);
        println!("collection: {}", collection_status);
    }
}
