//! A library for reading duplicity backups.
//!
//! This library provides utilities to manage duplicity backups [1]. Backup files could be prensent
//! in the local file system, or can be accessed remotely, provided that the right backend is
//! implemented. This is a rust version of the original duplicity project, that is written in
//! Python. The goal is to provide a library to be used for different purposes (e.g. a command
//! line utility, a fusion filesystem, etc.) and to improve overall performances. Compatibility
//! with the original duplicity backup format is guaranteed.
//!
//! [1]: http://duplicity.nongnu.org/
//!
//! # Example
//! In this example we open a directory containing a backup, and print informations about the files
//! in all the snapshots.
//!
//! ```
//! use ruplicity::Backup;
//! use ruplicity::backend::local::LocalBackend;
//! use ruplicity::timefmt::TimeDisplay;
//!
//! // use the local backend to open a path in the file system containing a backup
//! let backend = LocalBackend::new("tests/backups/single_vol");
//! let backup = Backup::new(backend).unwrap();
//! for snapshot in backup.snapshots().unwrap() {
//!     println!("Snapshot {}", snapshot.time().into_local_display());
//!     println!("{}", snapshot.entries().unwrap());
//! }
//! ```

#![deny(missing_copy_implementations,
        missing_docs,
        trivial_casts, trivial_numeric_casts,
        unstable_features,
        unsafe_code,
        unused_import_braces, unused_qualifications)]

#![cfg_attr(feature = "nightly", allow(unstable_features))]
#![cfg_attr(feature = "lints", feature(plugin))]
#![cfg_attr(feature = "lints", plugin(clippy))]

extern crate byteorder;
extern crate flate2;
extern crate fnv;
extern crate linked_hash_map;
extern crate regex;
extern crate tabwriter;
extern crate tar;
extern crate time;
#[macro_use]
extern crate try_opt;

mod macros;
mod rawpath;
mod read;

pub mod backend;
pub mod collections;
pub mod manifest;
pub mod signatures;
pub mod timefmt;

use std::cell::{Ref, RefCell};
use std::error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::ops::Deref;
use std::path::Path;

use time::Timespec;

pub use backend::Backend;
use collections::{BackupChain, BackupSet, Collections};
use manifest::Manifest;
use signatures::Chain;


/// A top level representation of a duplicity backup.
#[derive(Debug)]
pub struct Backup<B> {
    backend: B,
    collections: Collections,
    signatures: Vec<RefCell<Option<Chain>>>,
    manifests: Vec<RefCell<Option<Manifest>>>,
}

/// Represents all the snapshots in a backup.
pub struct Snapshots<'a> {
    backup: &'a ResourceCache,
}

/// An iterator over the snapshots in a backup.
pub struct SnapshotsIter<'a> {
    set_iter: CollectionsIter<'a>,
    chain_id: usize,
    sig_id: usize,
    man_id: usize,
    backup: &'a ResourceCache,
}

/// A snapshot in a backup.
pub struct Snapshot<'a> {
    set: &'a BackupSet,
    // the number of the parent backup chain, starting from zero
    chain_id: usize,
    sig_id: usize,
    man_id: usize,
    backup: &'a ResourceCache,
}

/// Contains the files present in a certain backup snapshot.
pub struct SnapshotEntries<'a> {
    chain: Ref<'a, Option<Chain>>,
    sig_id: usize,
}

/// Reference to a Manifest.
#[derive(Debug)]
pub struct ManifestRef<'a>(Ref<'a, Option<Manifest>>);


struct CollectionsIter<'a> {
    chain_iter: collections::ChainIter<'a, BackupChain>,
    incset_iter: Option<collections::BackupSetIter<'a>>,
}

/// Allows to be used as an interface for `Backup` struct without generic parameters. This allows
/// to reduce code size, since we don't have to godegen the entire module for different Backend
/// generic parameters. This trait is used as an interface between `Backup` and its inner
/// components.
trait ResourceCache {
    fn _collections(&self) -> &Collections;
    fn _signature_chain(&self, chain_id: usize) -> io::Result<Ref<Option<Chain>>>;
    fn _manifest(&self,
                 chain_id: usize,
                 manifest_path: &str)
                 -> Result<Ref<Option<Manifest>>, manifest::ParseError>;
}


impl<B: Backend> Backup<B> {
    /// Opens an existig backup by using the given backend.
    ///
    /// # Errors
    /// This function will return an error whenever the backend returns an error in a file
    /// operation. If the backend can't provide access to backup files, because they are
    /// unavailable or non-existing, an empty backup could be returned.
    ///
    /// # Examples
    /// ```
    /// use ruplicity::Backup;
    /// use ruplicity::backend::local::LocalBackend;
    ///
    /// // use the local backend to open a path in the file system containing a backup
    /// let backend = LocalBackend::new("tests/backups/single_vol");
    /// let backup = Backup::new(backend).unwrap();
    /// println!("Got backup with {} snapshots!", backup.snapshots().unwrap().into_iter().count());
    /// ```
    pub fn new(backend: B) -> io::Result<Self> {
        let files = try!(backend.file_names());
        let collections = Collections::from_filenames(files);
        let signatures = collections.signature_chains().map(|_| RefCell::new(None)).collect();
        let manifests = (0..collections.num_snapshots()).map(|_| RefCell::new(None)).collect();
        Ok(Backup {
            backend: backend,
            collections: collections,
            signatures: signatures,
            manifests: manifests,
        })
    }

    /// Constructs an iterator over the snapshots currently present in this backup.
    pub fn snapshots(&self) -> io::Result<Snapshots> {
        // in future, when we will add lazy collections,
        // this could fail, so we add a Result in advance
        Ok(Snapshots { backup: self })
    }

    /// Unwraps this backup and returns the inner backend.
    pub fn into_inner(self) -> B {
        self.backend
    }
}


impl<'a> Snapshots<'a> {
    /// Returns the low level representation of the snapshots.
    pub fn as_collections(&self) -> &'a Collections {
        self.backup._collections()
    }
}

impl<'a> IntoIterator for Snapshots<'a> {
    type Item = Snapshot<'a>;
    type IntoIter = SnapshotsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let set_iter = CollectionsIter {
            chain_iter: self.backup._collections().backup_chains(),
            incset_iter: None,
        };
        // in future, when we will add lazy collections,
        // this could fail, so we add a Result in advance
        SnapshotsIter {
            set_iter: set_iter,
            chain_id: 0,
            sig_id: 0,
            man_id: 0,
            backup: self.backup,
        }
    }
}


impl<'a> Iterator for SnapshotsIter<'a> {
    type Item = Snapshot<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // first test if we have a valid iterator to an incset
        if let Some(ref mut incset_iter) = self.set_iter.incset_iter {
            // we have a set iter, so return the next element if present
            if let Some(inc_set) = incset_iter.next() {
                self.sig_id += 1;
                self.man_id += 1;
                return Some(Snapshot {
                    set: inc_set,
                    chain_id: self.chain_id - 1,
                    sig_id: self.sig_id,
                    man_id: self.man_id - 1,
                    backup: self.backup,
                });
            }
        }
        // the current incset is exausted or not present,
        // we need to advance the chain and return the next full set if present,
        // otherwise the job is finished
        match self.set_iter.chain_iter.next() {
            Some(chain) => {
                self.chain_id += 1;
                self.sig_id = 0;
                self.man_id += 1;
                self.set_iter.incset_iter = Some(chain.inc_sets());
                Some(Snapshot {
                    set: chain.full_set(),
                    chain_id: self.chain_id - 1,
                    sig_id: self.sig_id,
                    man_id: self.man_id - 1,
                    backup: self.backup,
                })
            }
            None => None,
        }
    }
}


impl<'a> Snapshot<'a> {
    /// Returns the time in which the snapshot has been taken.
    pub fn time(&self) -> Timespec {
        self.set.end_time()
    }

    /// Returns whether the snapshot is a full backup.
    ///
    /// A full snapshot does not depend on previous snapshots.
    pub fn is_full(&self) -> bool {
        self.set.is_full()
    }

    /// Returns whether the snapshot is an incremental backup.
    ///
    /// An incremental snapshot depends on all the previous incremental snapshots and the first
    /// previous full snapshot. This set of dependent snapshots is called "chain".
    pub fn is_incremental(&self) -> bool {
        self.set.is_incremental()
    }

    /// Returns the number of volumes contained in the snapshot.
    pub fn num_volumes(&self) -> usize {
        self.set.num_volumes()
    }

    /// Returns the low level representation of the snapshot.
    pub fn as_backup_set(&self) -> &'a BackupSet {
        self.set
    }

    /// Returns the files and directories present in the snapshot.
    ///
    /// Be aware that using this functionality means that all the signature files in the current
    /// backup chain must be loaded, and this could take some time, depending on the file access
    /// provided by the backend and the signatures size.
    pub fn entries(&self) -> io::Result<SnapshotEntries> {
        let sig = try!(self.backup._signature_chain(self.chain_id));
        if self.sig_id < sig.as_ref().unwrap().snapshots().len() {
            Ok(SnapshotEntries {
                chain: sig,
                sig_id: self.sig_id,
            })
        } else {
            Err(not_found("The signature chain is incomplete"))
        }
    }

    /// Returns the manifest for this snapshot.
    ///
    /// The relative manifest file is read on demand and cached for subsequent uses.
    pub fn manifest(&self) -> Result<ManifestRef<'a>, manifest::ParseError> {
        Ok(ManifestRef(try!(self.backup._manifest(self.man_id, self.set.manifest_path()))))
    }
}


impl<'a> SnapshotEntries<'a> {
    /// Returns the signatures representation for the entries.
    ///
    /// This function can be used to retrieve information about the files in the snapshot.
    pub fn as_signature(&self) -> signatures::SnapshotEntries {
        self.chain.as_ref().unwrap().snapshots().nth(self.sig_id).unwrap().entries()
    }
}

impl<'a> Display for SnapshotEntries<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.as_signature().fmt(f)
    }
}


impl<'a> Deref for ManifestRef<'a> {
    type Target = Manifest;

    fn deref(&self) -> &Manifest {
        self.0.as_ref().unwrap()
    }
}


impl<B: Backend> ResourceCache for Backup<B> {
    fn _collections(&self) -> &Collections {
        &self.collections
    }

    fn _signature_chain(&self, chain_id: usize) -> io::Result<Ref<Option<Chain>>> {
        {
            // check if there is a cached value
            let mut sig = self.signatures[chain_id].borrow_mut();
            if sig.is_none() {
                // compute signatures now
                if let Some(sigchain) = self.collections.signature_chains().nth(chain_id) {
                    let new_sig = try!(Chain::from_sigchain(sigchain, &self.backend));
                    *sig = Some(new_sig);
                } else {
                    return Err(not_found("The given backup snapshot does not have a \
                                         corresponding signature"));
                }
            }
        }

        // need to close previous scope to borrow again
        // return the cached value
        Ok(self.signatures[chain_id].borrow())
    }

    fn _manifest(&self,
                 id: usize,
                 path: &str)
                 -> Result<Ref<Option<Manifest>>, manifest::ParseError> {
        {
            // check if there is a cached value
            let mut sig = self.manifests[id].borrow_mut();
            if sig.is_none() {
                // compute manifest now
                let mut file = io::BufReader::new(try!(self.backend.open_file(Path::new(path))));
                *sig = Some(try!(Manifest::parse(&mut file)));
            }
        }

        // need to close previous scope to borrow again
        // return the cached value
        Ok(self.manifests[id].borrow())
    }
}


fn io_err<E>(kind: io::ErrorKind, e: E) -> io::Error
    where E: Into<Box<error::Error + Send + Sync>>
{
    io::Error::new(kind, e)
}

fn not_found<E>(msg: E) -> io::Error
    where E: Into<Box<error::Error + Send + Sync>>
{
    io_err(io::ErrorKind::NotFound, msg)
}


#[cfg(test)]
mod test {
    use super::*;
    use backend::local::LocalBackend;
    use collections::{BackupSet, Collections};
    use manifest::Manifest;
    use rawpath::RawPath;
    use signatures::{Chain, Entry};
    use timefmt::parse_time_str;

    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;
    use time::Timespec;


    #[derive(Debug, Eq, PartialEq)]
    struct SnapshotTest {
        time: Timespec,
        is_full: bool,
        num_volumes: usize,
    }

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct EntryTest {
        path: RawPath,
        mtime: Timespec,
        uname: String,
        gname: String,
    }

    impl EntryTest {
        pub fn from_entry(file: &Entry) -> Self {
            EntryTest {
                path: RawPath::from_bytes(file.path_bytes().to_owned()),
                mtime: file.mtime(),
                uname: file.username().unwrap().to_owned(),
                gname: file.groupname().unwrap().to_owned(),
            }
        }

        pub fn from_info(path: &[u8], mtime: &str, uname: &str, gname: &str) -> Self {
            EntryTest {
                path: RawPath::from_bytes(path.to_owned()),
                mtime: parse_time_str(mtime).unwrap(),
                uname: uname.to_owned(),
                gname: gname.to_owned(),
            }
        }
    }

    fn from_backup_set(set: &BackupSet, full: bool) -> SnapshotTest {
        SnapshotTest {
            time: set.end_time(),
            is_full: full,
            num_volumes: set.num_volumes(),
        }
    }

    fn from_collection(coll: &Collections) -> Vec<SnapshotTest> {
        let mut result = Vec::new();
        for chain in coll.backup_chains() {
            result.push(from_backup_set(chain.full_set(), true));
            for set in chain.inc_sets() {
                result.push(from_backup_set(set, false));
            }
        }
        result
    }

    fn to_test_snapshot<B: Backend>(backup: &Backup<B>) -> Vec<SnapshotTest> {
        backup.snapshots()
            .unwrap()
            .into_iter()
            .map(|s| {
                assert!(s.is_full() != s.is_incremental());
                SnapshotTest {
                    time: s.time(),
                    is_full: s.is_full(),
                    num_volumes: s.num_volumes(),
                }
            })
            .collect()
    }

    fn single_vol_signature_chain() -> Chain {
        let backend = LocalBackend::new("tests/backups/single_vol");
        let filenames = backend.file_names().unwrap();
        let coll = Collections::from_filenames(filenames);
        Chain::from_sigchain(coll.signature_chains().next().unwrap(), &backend).unwrap()
    }

    fn from_sigchain(chain: &Chain) -> Vec<Vec<EntryTest>> {
        chain.snapshots()
            .map(|s| {
                s.entries()
                    .into_iter()
                    .map(|f| EntryTest::from_entry(&f))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }

    fn from_backup<B: Backend>(backup: &Backup<B>) -> Vec<Vec<EntryTest>> {
        backup.snapshots()
            .unwrap()
            .into_iter()
            .map(|s| {
                s.entries()
                    .unwrap()
                    .as_signature()
                    .into_iter()
                    .map(|f| EntryTest::from_entry(&f))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }


    #[test]
    fn same_collections_single_vol() {
        let backend = LocalBackend::new("tests/backups/single_vol");
        let filenames = backend.file_names().unwrap();
        let coll = Collections::from_filenames(filenames);
        let backup = Backup::new(backend).unwrap();

        let expected = from_collection(&coll);
        let actual = to_test_snapshot(&backup);
        assert_eq!(actual, expected);
    }

    #[test]
    fn same_collections_multi_chain() {
        let backend = LocalBackend::new("tests/backups/multi_chain");
        let filenames = backend.file_names().unwrap();
        let coll = Collections::from_filenames(filenames);
        let backup = Backup::new(backend).unwrap();

        let expected = from_collection(&coll);
        let actual = to_test_snapshot(&backup);
        assert_eq!(actual, expected);
    }

    #[test]
    fn same_files() {
        let sigchain = single_vol_signature_chain();
        let expected = from_sigchain(&sigchain);

        let backend = LocalBackend::new("tests/backups/single_vol");
        let backup = Backup::new(backend).unwrap();
        let actual = from_backup(&backup);
        assert_eq!(actual, expected);
    }

    #[test]
    fn multi_chain_files() {
        let backend = LocalBackend::new("tests/backups/multi_chain");
        let backup = Backup::new(backend).unwrap();
        let actual = from_backup(&backup);
        let expected = vec![vec![make_entry_test(b"", "20160108t223141z"),
                                 make_entry_test(b"file", "20160108t222924z")],
                            vec![make_entry_test(b"", "20160108t223153z"),
                                 make_entry_test(b"file", "20160108t223153z")],
                            vec![make_entry_test(b"", "20160108t223206z"),
                                 make_entry_test(b"file", "20160108t223206z")],
                            vec![make_entry_test(b"", "20160108t223215z"),
                                 make_entry_test(b"file", "20160108t223215z")]];
        assert_eq!(actual, expected);

        fn make_entry_test(path: &[u8], mtime: &str) -> EntryTest {
            EntryTest::from_info(path, mtime, "michele", "michele")
        }
    }

    #[test]
    fn multi_chain_manifests() {
        let backend = LocalBackend::new("tests/backups/multi_chain");
        let backup = Backup::new(backend).unwrap();
        let actual = backup.snapshots()
            .unwrap()
            .into_iter()
            .map(|snapshot| snapshot.manifest().unwrap());
        let names = vec!["duplicity-full.20160108T223144Z.manifest",
                         "duplicity-inc.20160108T223144Z.to.20160108T223159Z.manifest",
                         "duplicity-full.20160108T223209Z.manifest",
                         "duplicity-inc.20160108T223209Z.to.20160108T223217Z.manifest"];
        let expected = names.iter()
            .map(|name| {
                let mut path = Path::new("tests/backups/multi_chain").to_owned();
                path.push(name);
                let mut file = BufReader::new(File::open(path).unwrap());
                Manifest::parse(&mut file).unwrap()
            });
        for (e, a) in expected.zip(actual) {
            assert_eq!(e, *a);
        }
    }
}
