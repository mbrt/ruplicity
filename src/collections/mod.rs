// TODO: Make this non public
pub mod file_naming;

use time_utils;
use time_utils::to_pretty_local;
use std::collections::HashMap;
use std::fmt::{Display, Error, Formatter};
use std::slice;
use time::Timespec;
use self::file_naming::{FileName, FileNameInfo, FileType, FileNameParser};


pub struct BackupSet {
    pub file_type: FileType,
    pub time: Timespec,
    pub start_time: Timespec,
    pub end_time: Timespec,
    pub compressed: bool,
    pub encrypted: bool,
    pub partial: bool,
    pub manifest_path: String,
    volumes_paths: HashMap<i32, String>,
    info_set: bool,    // true if informations are set
}

pub struct BackupChain {
    pub fullset: BackupSet,
    pub incset_list: Vec<BackupSet>,
    pub start_time: Timespec,
    pub end_time: Timespec
}

pub struct SignatureFile {
    pub file_name: String,
    pub time: Timespec,
    pub compressed: bool,
    pub encrypted: bool
}

/// A chain of signature belonging to the same backup set.
pub struct SignatureChain {
    /// The file name of the full signature chain.
    pub fullsig: SignatureFile,
    /// A list of file names for incremental signatures.
    pub inclist: Vec<SignatureFile>
}

pub struct CollectionsStatus {
    backup_chains: Vec<BackupChain>,
    sig_chains: Vec<SignatureChain>
}

/// Iterator over some kind of chain
pub type ChainIter<'a, T> = slice::Iter<'a, T>;


// TODO: Remove
type FileNameInfos<'a> = Vec<FileNameInfo<'a>>;


impl BackupSet {
    // TODO: remake new like BackupChain, only with the starting filename and remove info_set.
    pub fn new() -> Self {
        BackupSet{
            file_type: FileType::Full,
            time: time_utils::DEFAULT_TIMESPEC,
            start_time: time_utils::DEFAULT_TIMESPEC,
            end_time: time_utils::DEFAULT_TIMESPEC,
            compressed: false,
            encrypted: false,
            partial: false,
            manifest_path: String::new(),
            volumes_paths: HashMap::new(),
            info_set: false
        }
    }

    /// Add a filename to given set. Return true if it fits.
    ///
    /// The filename will match the given set if it has the right
    /// times and is of the right type. The information will be set
    /// from the first filename given.
    pub fn add_filename(&mut self, file_info: &FileNameInfo) -> bool {
        let pr = &file_info.info;
        let fname = file_info.file_name;

        if !self.info_set {
            self.set_info(&pr);
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

    pub fn get_time(&self) -> &Timespec {
        if self.time == time_utils::DEFAULT_TIMESPEC {
            &self.end_time
        }
        else {
            &self.time
        }
    }

    fn set_info(&mut self, fname: &FileName) {
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
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self.file_type {
            FileType::Full => {
                try!(write!(f, "Full, time: {}", to_pretty_local(self.time)));
            },
            FileType::Inc => {
                try!(write!(f, "Incremental, start time: {}, end time: {}",
                            to_pretty_local(self.start_time),
                            to_pretty_local(self.end_time)));
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


impl BackupChain {
    /// Create a new BackupChain starting from a full backup set.
    pub fn new(fullset: BackupSet) -> Self {
        assert_eq!(fullset.file_type, FileType::Full);
        let time = fullset.time.clone();

        BackupChain{
            fullset: fullset,
            incset_list: Vec::new(),
            start_time: time,
            end_time: time
        }
    }

    /// Adds the given incremental backup element to the backup chain if possible,
    /// returns it back otherwise.
    pub fn add_inc(&mut self, incset: BackupSet) -> Option<BackupSet> {
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
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        try!(write!(f, "start time: {}, end time: {}\n{}",
                    to_pretty_local(self.start_time),
                    to_pretty_local(self.end_time),
                    &self.fullset));
        for inc in self.incset_list.iter() {
            try!(write!(f, "\n{}", inc));
        }
        Ok(())
    }
}


impl SignatureFile {
    pub fn from_file_and_info(fname: &str, pr: &FileName) -> Self {
        SignatureFile{
            file_name: fname.to_owned(),
            time: if pr.file_type == FileType::FullSig { pr.time } else { pr.end_time },
            compressed: pr.compressed,
            encrypted: pr.encrypted
        }
    }

    pub fn from_filename_info(info: &FileNameInfo) -> Self {
        Self::from_file_and_info(info.file_name, &info.info)
    }
}


impl SignatureChain {
    /// Create a new SignatureChain starting from a full signature.
    pub fn new(fname: &str, pr: &FileName) -> Self {
        SignatureChain {
            fullsig: SignatureFile::from_file_and_info(fname, pr),
            inclist: Vec::new()
        }
    }

    pub fn from_filename_info(fname_info: &FileNameInfo) -> Self {
        Self::new(fname_info.file_name, &fname_info.info)
    }

    /// Adds the given incremental signature to the signature chain if possible,
    /// returns false otherwise.
    pub fn add_new_sig(&mut self, fname: &FileNameInfo) -> bool {
        if fname.info.file_type != FileType::NewSig {
            false
        }
        else {
            self.inclist.push(SignatureFile::from_filename_info(fname));
            true
        }
    }

    pub fn start_time(&self) -> Timespec {
        self.fullsig.time
    }

    pub fn end_time(&self) -> Timespec {
        self.inclist.last().map_or(self.start_time(), |inc| inc.time)
    }
}

impl Display for SignatureChain {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        try!(write!(f, "start time: {}, end time: {}\n{}",
                    to_pretty_local(self.start_time()),
                    to_pretty_local(self.end_time()),
                    &self.fullsig.file_name));
        for inc in self.inclist.iter() {
            try!(write!(f, "\n{}", inc.file_name));
        }
        Ok(())
    }
}


impl CollectionsStatus {
    pub fn new() -> Self {
        CollectionsStatus {
            backup_chains: Vec::new(),
            sig_chains: Vec::new()
        }
    }

    pub fn from_filenames<T: AsRef<str>>(filenames: &[T]) -> Self {
        let mut result = Self::new();
        let filename_infos = Self::compute_filename_infos(&filenames);
        result.compute_backup_chains(&filename_infos);
        result.compute_signature_chains(&filename_infos);
        result
    }

    pub fn backup_chains(&self) -> ChainIter<BackupChain> {
        self.backup_chains.iter()
    }

    pub fn signature_chains(&self) -> ChainIter<SignatureChain> {
        self.sig_chains.iter()
    }

    fn compute_filename_infos<'a, T: AsRef<str>>(filename_list: &'a [T]) -> FileNameInfos<'a> {
        let mut result = Vec::new();
        let parser = FileNameParser::new();
        for name in filename_list {
            if let Some(info) = parser.parse(name.as_ref()) {
                result.push(FileNameInfo::new(name.as_ref(), info));
            }
        }
        result
    }

    fn compute_backup_chains(&mut self, filename_list: &FileNameInfos) {
        let sets = Self::compute_backup_sets(filename_list);
        self.add_to_backup_chains(sets);
    }

    fn compute_backup_sets(filename_list: &FileNameInfos) -> Vec<BackupSet> {
        let mut sets = Vec::<BackupSet>::new();
        for fileinfo in filename_list.iter() {
            let mut inserted = false;
            for set in sets.iter_mut() {
                if set.add_filename(&fileinfo) {
                    inserted = true;
                    break;
                }
            }
            if !inserted {
                let mut new_set = BackupSet::new();
                new_set.add_filename(&fileinfo);
                sets.push(new_set);
            }
        }
        // sort by time
        sets.sort_by(|a, b| a.get_time().cmp(b.get_time()));
        sets
    }

    fn add_to_backup_chains(&mut self, set_list: Vec<BackupSet>) {
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
        // sort by end time
        self.backup_chains.sort_by(|a, b| a.end_time.cmp(&b.end_time));
    }

    fn compute_signature_chains(&mut self, filename_list: &FileNameInfos) {
        // create a new signature chain for each fill signature
        self.sig_chains = filename_list.iter()
            .filter(|f| f.info.file_type == FileType::FullSig)
            .map(|f| SignatureChain::from_filename_info(f))
            .collect();
        // and collect all the new signatures, sorted by start time
        let mut new_sig: Vec<_> = filename_list.iter()
            .filter(|f| f.info.file_type == FileType::NewSig)
            .collect();
        new_sig.sort_by(|a, b| a.info.start_time.cmp(&b.info.start_time));

        // add the new signatures to signature chains
        for sig in new_sig.into_iter() {
            let mut added = false;
            for chain in self.sig_chains.iter_mut() {
                if chain.add_new_sig(&sig) {
                    added = true;
                    break;
                }
            }
            if !added {
                // TODO: add to orphaned filenames
            }
        }
    }
}

impl Display for CollectionsStatus {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        for backup_chain in &self.backup_chains {
            try!(backup_chain.fmt(f));
        }
        try!(write!(f, "\nsignature chains:\n"));
        for signature_chain in &self.sig_chains {
            try!(signature_chain.fmt(f));
        }
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use super::{BackupSet, CollectionsStatus};
    use super::file_naming::{FileType, FileNameInfo, FileNameParser};
    use time_utils::{DEFAULT_TIMESPEC, parse_time_str};

    #[test]
    fn parse_and_add() {
        let full1_name = "duplicity-full.20150617T182545Z.vol1.difftar.gz";
        let manifest1_name = "duplicity-full.20150617T182545Z.manifest";
        let inc1_name = "duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz";

        let parser = FileNameParser::new();
        let full1 = FileNameInfo::new(full1_name, parser.parse(full1_name).unwrap());
        let manifest1 = FileNameInfo::new(manifest1_name, parser.parse(manifest1_name).unwrap());
        let inc1 = FileNameInfo::new(inc1_name, parser.parse(inc1_name).unwrap());

        let mut set = BackupSet::new();
        // add to set
        assert!(set.add_filename(&full1));
        assert!(set.add_filename(&manifest1));
        assert!(!set.add_filename(&inc1));
        // test results
        assert_eq!(set.file_type, FileType::Full);
        assert_eq!(set.time, parse_time_str("20150617t182545z").unwrap());
        assert_eq!(set.start_time, DEFAULT_TIMESPEC);
        assert_eq!(set.end_time, DEFAULT_TIMESPEC);
        assert!(set.compressed);
        assert!(!set.encrypted);
        assert!(!set.partial);
        assert_eq!(set.manifest_path, manifest1_name);
    }

    #[test]
    fn collection_status() {
        let filenames = vec![
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
        let collection_status = CollectionsStatus::from_filenames(&filenames);
        println!("collection: {}", collection_status);
    }
}
