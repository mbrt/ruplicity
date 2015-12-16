//! Operations on backup files.
//!
//! This sub-module provides information about a backup, by looking at the files present in a
//! backup directory.

mod file_naming;

use std::collections::HashMap;
use std::fmt::{Display, Error, Formatter};
use std::path::Path;
use std::slice;
use time::Timespec;

use time_utils::TimeDisplay;
use self::file_naming as fnm;
use self::file_naming::{FileNameInfo, FileNameParser};


/// General information about a backup.
///
/// Determines the status of a backup by looking at the files present in the backup folder. No
/// backup archive is opened in this process. Thanks to that, performances are great; however no
/// validation is performed on backup files.
#[derive(Debug)]
pub struct Collections {
    backup_chains: Vec<BackupChain>,
    sig_chains: Vec<SignatureChain>,
}

/// Contains information about a backup chain.
///
/// A backup chain is composed by one full and all the incremental backup snapshots before the
/// next full one.
#[derive(Debug)]
pub struct BackupChain {
    fullset: BackupSet,
    incsets: Vec<BackupSet>,
    start_time: Timespec,
    end_time: Timespec,
}

/// Contains information about signatures in a backup chain.
///
/// See the docs for [`BackupChain`](struct.BackupChain.html).
#[derive(Debug)]
pub struct SignatureChain {
    fullsig: SignatureFile,
    incsigs: Vec<SignatureFile>,
}

/// Information about a backup snapshot.
#[derive(Debug)]
pub struct BackupSet {
    tp: Type,
    compressed: bool,
    encrypted: bool,
    partial: bool,
    manifest_path: String,
    volumes_paths: HashMap<i32, String>,
}

/// Information about a signature file.
#[derive(Debug)]
pub struct SignatureFile {
    pub file_name: String,
    pub time: Timespec,
    pub compressed: bool,
    pub encrypted: bool,
}

/// Iterator over some kind of chain.
pub type ChainIter<'a, T> = slice::Iter<'a, T>;

/// Iterator over `BackupSet`s.
pub type BackupSetIter<'a> = slice::Iter<'a, BackupSet>;

/// Iterator over `SignatureFile`s.
pub type SignatureFileIter<'a> = slice::Iter<'a, SignatureFile>;


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Type {
    Full {
        time: Timespec,
    },
    Inc {
        start_time: Timespec,
        end_time: Timespec,
    },
}


impl BackupSet {
    /// Creates a new `BackupSet`, starting from file name information.
    pub fn new(fname: &FileNameInfo) -> Self {
        // set type
        let tp = match fname.info.tp {
            fnm::Type::Full{ time, .. } |
            fnm::Type::FullManifest{ time, .. } |
            fnm::Type::FullSig{ time, .. } => Type::Full { time: time },
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
                fnm::Type::NewSig{ partial, .. } => partial,
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

        if !self.is_same_set(&pr) {
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

    pub fn start_time(&self) -> Timespec {
        self.tp.start_time()
    }

    pub fn end_time(&self) -> Timespec {
        self.tp.end_time()
    }

    pub fn is_compressed(&self) -> bool {
        self.compressed
    }

    pub fn is_encrypted(&self) -> bool {
        self.encrypted
    }

    pub fn is_partial(&self) -> bool {
        self.partial
    }

    pub fn manifest_path(&self) -> &str {
        self.manifest_path.as_ref()
    }

    pub fn volume_path(&self, volume_num: i32) -> Option<&str> {
        self.volumes_paths.get(&volume_num).map(AsRef::as_ref)
    }

    /// Checks if the given file belongs to the same backup set, by looking at timestamps.
    pub fn is_same_set(&self, pr: &fnm::Info) -> bool {
        match self.tp {
            Type::Full{ time: my_time } => {
                match pr.tp {
                    fnm::Type::Full{ time, .. } |
                    fnm::Type::FullManifest{ time, .. } |
                    fnm::Type::FullSig{ time, .. } => my_time == time,
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
    }

    pub fn is_full(&self) -> bool {
        matches!(self.tp, Type::Full{..})
    }

    pub fn is_incremental(&self) -> bool {
        matches!(self.tp, Type::Inc{..})
    }

    pub fn num_volumes(&self) -> usize {
        self.volumes_paths.len()
    }

    fn fix_encrypted(&mut self, pr_encrypted: bool) {
        if self.encrypted != pr_encrypted && self.partial && pr_encrypted {
            self.encrypted = pr_encrypted;
        }
    }
}

impl Display for BackupSet {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let tp = match self.tp {
            Type::Full{ .. } => "Full",
            Type::Inc{ .. } => "Incremental",
        };
        write!(f,
               "{:<20} {:<13} {:>12}",
               tp,
               // FIXME: Workaround for rust <= 1.4
               // Alignment is ignored by custom formatters
               // see: https://github.com/rust-lang-deprecated/time/issues/98#issuecomment-103010106
               format!("{}", self.end_time().into_local_display()),
               self.num_volumes())
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
            incsets: Vec::new(),
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
                self.incsets.push(incset);
                None
            } else {
                // replace the last element if the end time comes before
                let replace_last = self.incsets.last().map_or(false, |last| {
                    start_time == last.tp.start_time() && end_time > last.tp.end_time()
                });
                if replace_last {
                    self.end_time = end_time.clone();
                    self.incsets.pop();
                    self.incsets.push(incset);
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

    pub fn full_set(&self) -> &BackupSet {
        &self.fullset
    }

    pub fn inc_sets(&self) -> BackupSetIter {
        self.incsets.iter()
    }

    pub fn start_time(&self) -> Timespec {
        self.start_time
    }

    pub fn end_time(&self) -> Timespec {
        self.end_time
    }
}

impl Display for BackupChain {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let num_vol = self.fullset.volumes_paths.len() +
                      self.incsets
                          .iter()
                          .map(|i| i.volumes_paths.len())
                          .fold(0, |a, i| a + i);
        try!(write!(f,
                    "Chain start time: {}\n\
                    Chain end time: {}\n\
                    Number of contained backup sets: {}\n\
                    Total number of contained volumes: {}\n",
                    self.start_time.into_local_display(),
                    self.end_time.into_local_display(),
                    self.incsets.len() + 1,
                    num_vol));
        try!(write!(f,
                    "{:<20} {:<13} {:>12}",
                    "Type of backup set:",
                    "Time:",
                    "Num volumes:\n"));
        try!(write!(f, "{}\n", self.fullset));
        for inc in &self.incsets {
            try!(write!(f, "{}\n", inc));
        }
        Ok(())
    }
}


impl SignatureFile {
    pub fn from_file_and_info(fname: &str, pr: &fnm::Info) -> Self {
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
    pub fn new(fname: &str, pr: &fnm::Info) -> Self {
        SignatureChain {
            fullsig: SignatureFile::from_file_and_info(fname, pr),
            incsigs: Vec::new(),
        }
    }

    pub fn from_filename_info(fname_info: &FileNameInfo) -> Self {
        Self::new(fname_info.file_name, &fname_info.info)
    }

    /// Adds the given incremental signature to the signature chain if possible,
    /// returns false otherwise.
    pub fn add_new_sig(&mut self, fname: &FileNameInfo) -> bool {
        if let fnm::Type::NewSig{ .. } = fname.info.tp {
            self.incsigs.push(SignatureFile::from_filename_info(fname));
            true
        } else {
            false
        }
    }

    /// The file name of the full signature chain.
    pub fn full_signature(&self) -> &SignatureFile {
        &self.fullsig
    }

    /// A list of file names for incremental signatures.
    pub fn inc_signatures(&self) -> SignatureFileIter {
        self.incsigs.iter()
    }

    pub fn start_time(&self) -> Timespec {
        self.fullsig.time
    }

    pub fn end_time(&self) -> Timespec {
        self.incsigs.last().map_or(self.start_time(), |inc| inc.time)
    }
}

impl Display for SignatureChain {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        try!(write!(f,
                    "start time: {}, end time: {}\n {}\n",
                    self.start_time().into_local_display(),
                    self.end_time().into_local_display(),
                    &self.fullsig.file_name));
        for inc in &self.incsigs {
            try!(write!(f, " {}\n", inc.file_name));
        }
        Ok(())
    }
}


impl Collections {
    /// Creates a new empty collection.
    pub fn new() -> Self {
        Collections {
            backup_chains: Vec::new(),
            sig_chains: Vec::new(),
        }
    }

    /// Creates a collection, starting from a list of file names.
    ///
    /// The given file names are not opened for validation. Information is collected based solely
    /// by the names themselves.
    ///
    /// # Examples
    /// ```
    /// use ruplicity::collections::Collections;
    ///
    /// let names = vec!["duplicity-full.20150617T182545Z.manifest",
    ///                  "duplicity-full.20150617T182545Z.vol1.difftar.gz",
    ///                  "duplicity-full-signatures.20150617T182545Z.sigtar.gz",
    ///                  "duplicity-inc.20150617T182545Z.to.20150617T182629Z.manifest",
    ///                  "duplicity-inc.20150617T182545Z.to.20150617T182629Z.vol1.difftar.gz",
    ///                  "duplicity-inc.20150617T182629Z.to.20150617T182650Z.manifest"];
    /// let collections = Collections::from_filenames(&names);
    /// assert_eq!(collections.backup_chains().count(), 1);
    /// assert_eq!(collections.signature_chains().count(), 1);
    /// ```
    pub fn from_filenames<I>(filenames: I) -> Self
        where I: IntoIterator,
              I::Item: AsRef<Path>
    {
        let fnames_vec: Vec<_> = filenames.into_iter().collect();
        let infos = compute_filename_infos(&fnames_vec);
        Collections {
            backup_chains: compute_backup_chains(&infos),
            sig_chains: compute_signature_chains(&infos),
        }
    }

    /// Returns the backup chains.
    ///
    /// Each backup chain should be coupled with a signature chain. They can be matched because
    /// they are both in chronological order.
    pub fn backup_chains(&self) -> ChainIter<BackupChain> {
        self.backup_chains.iter()
    }

    /// Returns the signature chains.
    ///
    /// Each signature chain should be coupled with a backup chain. They can be matched because
    /// they are both in chronological order.
    pub fn signature_chains(&self) -> ChainIter<SignatureChain> {
        self.sig_chains.iter()
    }

}

fn compute_filename_infos<'a, I, E>(filenames: I) -> Vec<FileNameInfo<'a>>
    where I: IntoIterator<Item = &'a E>,
          E: AsRef<Path> + 'a
{
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
    for fileinfo in fname_infos {
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
    sets.sort_by(|a, b| a.end_time().cmp(&b.end_time()));
    sets
}

fn compute_signature_chains(fname_infos: &[FileNameInfo]) -> Vec<SignatureChain> {
    // create a new signature chain for each fill signature
    let mut sig_chains = fname_infos.iter()
                                    .filter(|f| matches!(f.info.tp, fnm::Type::FullSig{..}))
                                    .map(|f| SignatureChain::from_filename_info(f))
                                    .collect::<Vec<_>>();
    // and collect all the new signatures, sorted by start time
    let mut new_sig = fname_infos.iter()
                                 .filter(|f| matches!(f.info.tp, fnm::Type::NewSig{..}))
                                 .collect::<Vec<_>>();
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

impl Display for Collections {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        for backup_chain in &self.backup_chains {
            try!(backup_chain.fmt(f));
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

    fn get_test_filenames() -> Vec<&'static str> {
        vec!["duplicity-full.20150617T182545Z.manifest",
             "duplicity-full.20150617T182545Z.vol1.difftar.gz",
             "duplicity-full-signatures.20150617T182545Z.sigtar.gz",
             "duplicity-inc.20150617T182545Z.to.20150617T182629Z.manifest",
             "duplicity-inc.20150617T182545Z.to.20150617T182629Z.vol1.difftar.gz",
             "duplicity-inc.20150617T182629Z.to.20150617T182650Z.manifest",
             "duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz",
             "duplicity-new-signatures.20150617T182545Z.to.20150617T182629Z.sigtar.gz",
             "duplicity-new-signatures.20150617T182629Z.to.20150617T182650Z.sigtar.gz"]
    }

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
        assert!(set.is_full());
        assert_eq!(set.end_time(), parse_time_str("20150617t182545z").unwrap());
        assert!(set.compressed);
        assert!(!set.encrypted);
        assert!(!set.partial);
        assert_eq!(set.manifest_path, manifest1_name);
    }

    #[test]
    fn collection_status_display() {
        // NOTE: this is actually not a proper test
        //       here we are only printing out the snapshots.
        //       however not panicking is already something :)
        //       Display is not properly testable due to time zones differencies;
        //       we want to avoid using global mutexes in test code
        let filenames = get_test_filenames();
        let collection_status = Collections::from_filenames(&filenames);
        let display = format!("{}\n", collection_status);
        // println!("debug:\n{:?}\n", collection_status);
        println!("collection status:\n{}\n", display);
    }

    #[test]
    fn collection_status() {
        let filenames = get_test_filenames();
        let collection_status = Collections::from_filenames(&filenames);
        assert_eq!(collection_status.backup_chains().count(), 1);
        assert_eq!(collection_status.signature_chains().count(), 1);
        // backup chain
        let backup_chain = collection_status.backup_chains().next().unwrap();
        assert_eq!(backup_chain.incsets.len(), 2);
        assert_eq!(backup_chain.start_time,
                   parse_time_str("20150617t182545z").unwrap());
        assert_eq!(backup_chain.end_time,
                   parse_time_str("20150617t182650z").unwrap());
        // full backup
        let full = &backup_chain.fullset;
        assert!(full.is_full());
        assert_eq!(full.end_time(), parse_time_str("20150617t182545z").unwrap());
        // inc backups
        {
            let inc = &backup_chain.incsets[0];
            assert!(inc.is_incremental());
            assert_eq!(inc.start_time(),
                       parse_time_str("20150617t182545z").unwrap());
            assert_eq!(inc.end_time(), parse_time_str("20150617t182629z").unwrap());
        }
        {
            let inc = &backup_chain.incsets[1];
            assert!(inc.is_incremental());
            assert_eq!(inc.start_time(),
                       parse_time_str("20150617t182629z").unwrap());
            assert_eq!(inc.end_time(), parse_time_str("20150617t182650z").unwrap());
        }
    }
}
