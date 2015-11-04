mod file_naming;

use std::collections::HashMap;
use std::fmt::{Display, Error, Formatter};
use std::path::Path;
use std::slice;
use time::Timespec;

use time_utils::to_pretty_local;
use self::file_naming::{FileNameInfo, FileNameParser, Info};
use self::file_naming as fnm;


pub struct BackupSet {
    pub tp: Type,
    pub compressed: bool,
    pub encrypted: bool,
    pub partial: bool,
    pub manifest_path: String,
    volumes_paths: HashMap<i32, String>,
}

pub struct BackupChain {
    pub fullset: BackupSet,
    pub incset_list: Vec<BackupSet>,
    pub start_time: Timespec,
    pub end_time: Timespec,
}

pub struct SignatureFile {
    pub file_name: String,
    pub time: Timespec,
    pub compressed: bool,
    pub encrypted: bool,
}

/// A chain of signature belonging to the same backup set.
pub struct SignatureChain {
    /// The file name of the full signature chain.
    pub fullsig: SignatureFile,
    /// A list of file names for incremental signatures.
    pub inclist: Vec<SignatureFile>,
}

pub struct CollectionsStatus {
    backup_chains: Vec<BackupChain>,
    sig_chains: Vec<SignatureChain>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Type {
    Full {
        time: Timespec,
    },
    Inc {
        start_time: Timespec,
        end_time: Timespec,
    },
}

/// Iterator over some kind of chain
pub type ChainIter<'a, T> = slice::Iter<'a, T>;


impl BackupSet {
    pub fn new(fname: &FileNameInfo) -> Self {
        // set type
        let tp = match fname.info.tp {
            fnm::Type::Full{ time, .. } |
            fnm::Type::FullManifest{ time, .. } |
            fnm::Type::FullSig{ time, .. } => {
                Type::Full { time: time }
            }
            fnm::Type::Inc{ start_time, end_time, .. } |
            fnm::Type::IncManifest{ start_time, end_time, .. } |
            fnm::Type::NewSig{ start_time, end_time, .. } => {
                Type::Inc {
                    start_time: start_time,
                    end_time: end_time,
                }
            }
        };
        // set partial
        let partial = {
            match fname.info.tp {
                fnm::Type::FullManifest{ partial, .. } |
                fnm::Type::IncManifest{ partial, .. } |
                fnm::Type::FullSig{ partial, .. } |
                fnm::Type::NewSig{ partial, .. } => {
                    partial
                }
                _ => false,
            }
        };

        let mut result = BackupSet {
            tp: tp,
            partial: partial,
            compressed: fname.info.compressed,
            encrypted: fname.info.encrypted,
            manifest_path: String::new(),
            volumes_paths: HashMap::new(),
        };
        result.add_filename(fname);
        result
    }

    /// Add a filename to given set. Return true if it fits.
    ///
    /// The filename will match the given set if it has the right
    /// times and is of the right type. The information will be set
    /// from the first filename given.
    pub fn add_filename(&mut self, file_info: &FileNameInfo) -> bool {
        let pr = &file_info.info;
        let fname = file_info.file_name;

        // check if same backup set, by looking at timestamps
        let same_set = {
            match self.tp {
                Type::Full{ time: my_time } => {
                    match pr.tp {
                        fnm::Type::Full{ time, .. } |
                        fnm::Type::FullManifest{ time, .. } |
                        fnm::Type::FullSig{ time, .. } => {
                            my_time == time
                        }
                        _ => false,
                    }
                }
                Type::Inc{ start_time: my_start, end_time: my_end } => {
                    match pr.tp {
                        fnm::Type::Inc{ start_time, end_time, .. } |
                        fnm::Type::IncManifest{ start_time, end_time, .. } |
                        fnm::Type::NewSig{ start_time, end_time, .. } => {
                            my_start == start_time && my_end == end_time
                        }
                        _ => false,
                    }
                }
            }
        };
        if !same_set {
            false
        } else {
            // update info
            match pr.tp {
                fnm::Type::Full{ volume_number, .. } |
                fnm::Type::Inc{ volume_number, .. } => {
                    self.volumes_paths.insert(volume_number, fname.to_owned());
                }
                fnm::Type::FullManifest{ .. } |
                fnm::Type::IncManifest{ .. } => {
                    self.manifest_path = fname.to_owned();
                }
                _ => (),
            }
            self.fix_encrypted(pr.encrypted);
            true
        }
    }

    pub fn is_complete(&self) -> bool {
        !self.manifest_path.is_empty()
    }

    pub fn get_time(&self) -> Timespec {
        match self.tp {
            Type::Full{ time } => time,
            Type::Inc{ end_time, .. } => end_time,
        }
    }

    fn fix_encrypted(&mut self, pr_encrypted: bool) {
        if self.encrypted != pr_encrypted && self.partial && pr_encrypted {
            self.encrypted = pr_encrypted;
        }
    }
}

impl Display for BackupSet {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self.tp {
            Type::Full{ time } => {
                try!(write!(f, "Full, time: {}", to_pretty_local(time)));
            }
            Type::Inc{ start_time, end_time } => {
                try!(write!(f,
                            "Incremental, start time: {}, end time: {}",
                            to_pretty_local(start_time),
                            to_pretty_local(end_time)));
            }
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
            for (_, vol) in &self.volumes_paths {
                try!(write!(f, " {}", vol));
            }
        }
        Ok(())
    }
}


impl BackupChain {
    /// Create a new BackupChain starting from a full backup set.
    pub fn new(fullset: BackupSet) -> Self {
        let time = {
            if let Type::Full{ time } = fullset.tp {
                time.clone()
            } else {
                panic!("Unexpected incremental backup set given");
            }
        };

        BackupChain {
            fullset: fullset,
            incset_list: Vec::new(),
            start_time: time,
            end_time: time,
        }
    }

    /// Adds the given incremental backup element to the backup chain if possible,
    /// returns it back otherwise.
    pub fn add_inc(&mut self, incset: BackupSet) -> Option<BackupSet> {
        if let Type::Inc{ start_time, end_time } = incset.tp {
            if self.end_time == start_time {
                self.end_time = end_time.clone();
                self.incset_list.push(incset);
                None
            } else {
                // replace the last element if the end time comes before
                let replace_last = self.incset_list.last().map_or(false, |last| {
                    start_time == last.tp.start_time() && end_time > last.tp.end_time()
                });
                if replace_last {
                    self.end_time = end_time.clone();
                    self.incset_list.pop();
                    self.incset_list.push(incset);
                    None
                } else {
                    // ignore the given incremental backup set
                    Some(incset)
                }
            }
        } else {
            // ignore full sets
            Some(incset)
        }
    }
}

impl Display for BackupChain {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        try!(write!(f,
                    "start time: {}, end time: {}\n{}",
                    to_pretty_local(self.start_time),
                    to_pretty_local(self.end_time),
                    &self.fullset));
        for inc in &self.incset_list {
            try!(write!(f, "\n{}", inc));
        }
        Ok(())
    }
}


impl SignatureFile {
    pub fn from_file_and_info(fname: &str, pr: &Info) -> Self {
        let time = {
            match pr.tp {
                fnm::Type::FullSig{ time, .. } => time,
                fnm::Type::NewSig{ end_time, .. } => end_time,
                _ => panic!("unexpected file given for signature"),
            }
        };
        SignatureFile {
            file_name: fname.to_owned(),
            time: time,
            compressed: pr.compressed,
            encrypted: pr.encrypted,
        }
    }

    pub fn from_filename_info(info: &FileNameInfo) -> Self {
        Self::from_file_and_info(info.file_name, &info.info)
    }
}


impl SignatureChain {
    /// Create a new SignatureChain starting from a full signature.
    pub fn new(fname: &str, pr: &Info) -> Self {
        SignatureChain {
            fullsig: SignatureFile::from_file_and_info(fname, pr),
            inclist: Vec::new(),
        }
    }

    pub fn from_filename_info(fname_info: &FileNameInfo) -> Self {
        Self::new(fname_info.file_name, &fname_info.info)
    }

    /// Adds the given incremental signature to the signature chain if possible,
    /// returns false otherwise.
    pub fn add_new_sig(&mut self, fname: &FileNameInfo) -> bool {
        if let fnm::Type::NewSig{ .. } = fname.info.tp {
            self.inclist.push(SignatureFile::from_filename_info(fname));
            true
        } else {
            false
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
        try!(write!(f,
                    "start time: {}, end time: {}\n{}",
                    to_pretty_local(self.start_time()),
                    to_pretty_local(self.end_time()),
                    &self.fullsig.file_name));
        for inc in &self.inclist {
            try!(write!(f, "\n{}", inc.file_name));
        }
        Ok(())
    }
}


impl CollectionsStatus {
    pub fn new() -> Self {
        CollectionsStatus {
            backup_chains: Vec::new(),
            sig_chains: Vec::new(),
        }
    }

    pub fn from_filenames<T: AsRef<Path>>(filenames: &[T]) -> Self {
        let infos = compute_filename_infos(&filenames);
        CollectionsStatus {
            backup_chains: compute_backup_chains(&infos),
            sig_chains: compute_signature_chains(&infos),
        }
    }

    pub fn backup_chains(&self) -> ChainIter<BackupChain> {
        self.backup_chains.iter()
    }

    pub fn signature_chains(&self) -> ChainIter<SignatureChain> {
        self.sig_chains.iter()
    }

}

fn compute_filename_infos<T: AsRef<Path>>(filenames: &[T]) -> Vec<FileNameInfo> {
    let mut result = Vec::new();
    let parser = FileNameParser::new();
    for name in filenames {
        if let Some(name) = name.as_ref().to_str() {
            if let Some(info) = parser.parse(name) {
                result.push(FileNameInfo::new(name, info));
            }
        }
    }
    result
}

fn compute_backup_chains(fname_infos: &[FileNameInfo]) -> Vec<BackupChain> {
    let mut backup_chains: Vec<BackupChain> = Vec::new();
    for set in compute_backup_sets(fname_infos) {
        match set.tp {
            Type::Full{ .. } => {
                let new_chain = BackupChain::new(set);
                backup_chains.push(new_chain);
            }
            Type::Inc{ .. } => {
                let mut rejected_set = Some(set);
                for chain in &mut backup_chains {
                    rejected_set = chain.add_inc(rejected_set.unwrap());
                    if rejected_set.is_none() {
                        break;
                    }
                }
                if let Some(_) = rejected_set {
                    // TODO: add to orphaned sets
                }
            }
        }
    }
    // sort by end time
    backup_chains.sort_by(|a, b| a.end_time.cmp(&b.end_time));
    backup_chains
}

fn compute_backup_sets(fname_infos: &[FileNameInfo]) -> Vec<BackupSet> {
    let mut sets = Vec::<BackupSet>::new();
    for fileinfo in fname_infos.iter() {
        let mut inserted = false;
        for set in &mut sets {
            if set.add_filename(&fileinfo) {
                inserted = true;
                break;
            }
        }
        if !inserted {
            sets.push(BackupSet::new(&fileinfo));
        }
    }
    // sort by time
    sets.sort_by(|a, b| a.get_time().cmp(&b.get_time()));
    sets
}

fn compute_signature_chains(fname_infos: &[FileNameInfo]) -> Vec<SignatureChain> {
    // create a new signature chain for each fill signature
    let mut sig_chains: Vec<_> = fname_infos.iter()
                                            .filter(|f| {
                                                matches!(f.info.tp, fnm::Type::FullSig{..})
                                            })
                                            .map(|f| SignatureChain::from_filename_info(f))
                                            .collect();
    // and collect all the new signatures, sorted by start time
    let mut new_sig: Vec<_> = fname_infos.iter()
                                         .filter(|f| matches!(f.info.tp, fnm::Type::NewSig{..}))
                                         .collect();
    new_sig.sort_by(|a, b| a.info.tp.time_range().0.cmp(&b.info.tp.time_range().0));

    // add the new signatures to signature chains
    for sig in new_sig.into_iter() {
        let mut added = false;
        for chain in &mut sig_chains {
            if chain.add_new_sig(&sig) {
                added = true;
                break;
            }
        }
        if !added {
            // TODO: add to orphaned filenames
        }
    }
    sig_chains
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


impl Type {
    pub fn start_time(&self) -> Timespec {
        match *self {
            Type::Full{ time } => time,
            Type::Inc{ start_time, .. } => start_time,
        }
    }

    pub fn end_time(&self) -> Timespec {
        match *self {
            Type::Full{ time } => time,
            Type::Inc{ end_time, .. } => end_time,
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use super::file_naming::{FileNameInfo, FileNameParser};
    use time_utils::parse_time_str;

    #[test]
    fn parse_and_add() {
        let full1_name = "duplicity-full.20150617T182545Z.vol1.difftar.gz";
        let manifest1_name = "duplicity-full.20150617T182545Z.manifest";
        let inc1_name = "duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz";

        let parser = FileNameParser::new();
        let full1 = FileNameInfo::new(full1_name, parser.parse(full1_name).unwrap());
        let manifest1 = FileNameInfo::new(manifest1_name, parser.parse(manifest1_name).unwrap());
        let inc1 = FileNameInfo::new(inc1_name, parser.parse(inc1_name).unwrap());

        let mut set = BackupSet::new(&full1);
        // add to set
        assert!(set.add_filename(&manifest1));
        assert!(!set.add_filename(&inc1));
        // test results
        assert_eq!(set.tp,
                   Type::Full { time: parse_time_str("20150617t182545z").unwrap() });
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

        // Expected output from duplicity:
        //
        // Chain start time: Wed Jun 17 18:25:45 2015
        // Chain end time: Wed Jun 17 18:26:50 2015
        // Number of contained backup sets: 3
        // Total number of contained volumes: 3
        //  Type of backup set:                            Time:      Num volumes:
        //                 Full         Wed Jun 17 18:25:45 2015                 1
        //          Incremental         Wed Jun 17 18:26:29 2015                 1
        //          Incremental         Wed Jun 17 18:26:50 2015                 1
    }
}
