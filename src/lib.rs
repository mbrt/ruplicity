//! A library for reading duplicity backups.
//!
//! This library provides utilities to manage duplicity backups [1]. Backup files could be prensent
//! in the local file system, or can be accessed remotely, provided that the right backend is
//! implemented. This is a rust version of the original duplicity project, that is written in
//! Python. The goal is to o provide a library to be used for different purposes (e.g. a command
//! line utility, a fusion filesystem, etc.) and to improve overall performances. Full
//! compatibility with the original duplicity is guaranteed.
//!
//! [1] http://duplicity.nongnu.org/

#![deny(missing_copy_implementations,
        trivial_casts, trivial_numeric_casts,
        unsafe_code,
        unstable_features,
        unused_import_braces, unused_qualifications)]

#![cfg_attr(feature = "nightly", allow(unstable_features))]
#![cfg_attr(feature = "lints", feature(plugin))]
#![cfg_attr(feature = "lints", plugin(clippy))]

extern crate byteorder;
extern crate flate2;
extern crate regex;
extern crate tabwriter;
extern crate tar;
extern crate time;
#[macro_use]
extern crate try_opt;

mod macros;
mod time_utils;
pub mod backend;
pub mod collections;
pub mod signatures;

use std::cell::{Ref, RefCell};
use std::io;

use time::Timespec;

pub use backend::Backend;
use collections::{BackupChain, BackupSet, Collections};
use signatures::Chain;


/// A top level representation of a duplicity backup.
#[derive(Debug)]
pub struct Backup<B> {
    backend: B,
    collections: Collections,
    signatures: Vec<RefCell<Option<Chain>>>,
}

/// An iterator over the snapshots in a backup.
pub struct Snapshots<'a> {
    set_iter: CollectionsIter<'a>,
    chain_id: usize,
    sig_id: usize,
    backup: &'a ResourceCache,
}

/// A snapshot in a backup.
pub struct Snapshot<'a> {
    set: &'a BackupSet,
    // the number of the parent backup chain, starting from zero
    chain_id: usize,
    sig_id: usize,
    backup: &'a ResourceCache,
}

/// Contains the files present in a certain backup snapshot.
pub struct SnapshotFiles<'a> {
    chain: Ref<'a, Option<Chain>>,
    sig_id: usize,
}


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
    /// ```no_run
    /// use ruplicity::{Backend, Backup};
    /// use ruplicity::backend::local::LocalBackend;
    ///
    /// // use the local backend to open a path in the file system containing a backup
    /// let backend = LocalBackend::new("/path/to/backup");
    /// let backup = Backup::new(backend);
    /// ```
    pub fn new(backend: B) -> io::Result<Self> {
        let files = try!(backend.get_file_names());
        let collections = Collections::from_filenames(files);
        let signatures = {
            // initialize signatures with empty signatures
            // to be loaded lazily
            let mut signatures = Vec::new();
            for _ in collections.signature_chains() {
                signatures.push(RefCell::new(None));
            }
            signatures
        };
        Ok(Backup {
            backend: backend,
            collections: collections,
            signatures: signatures,
        })
    }

    /// Constructs an iterator over the snapshots currently present in this backup.
    pub fn snapshots(&self) -> io::Result<Snapshots> {
        let set_iter = CollectionsIter {
            chain_iter: self.collections.backup_chains(),
            incset_iter: None,
        };
        // in future, when we will add lazy collections,
        // this could fail, so we add a Result in advance
        Ok(Snapshots {
            set_iter: set_iter,
            chain_id: 0,
            sig_id: 0,
            backup: self,
        })
    }
}


impl<'a> Iterator for Snapshots<'a> {
    type Item = Snapshot<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // first test if we have a valid iterator to an incset
        if let Some(ref mut incset_iter) = self.set_iter.incset_iter {
            // we have a set iter, so return the next element if present
            if let Some(inc_set) = incset_iter.next() {
                self.sig_id += 1;
                return Some(Snapshot {
                    set: inc_set,
                    chain_id: self.chain_id - 1,
                    sig_id: self.sig_id,
                    backup: self.backup,
                });
            }
        }
        // the current incset is exausted or not present,
        // we need to advance the chain and return the next full set if present,
        // otherwise the job is finished
        if let Some(chain) = self.set_iter.chain_iter.next() {
            self.chain_id += 1;
            self.sig_id = 0;
            self.set_iter.incset_iter = Some(chain.inc_sets());
            Some(Snapshot {
                set: chain.full_set(),
                chain_id: self.chain_id - 1,
                sig_id: self.sig_id,
                backup: self.backup,
            })
        } else {
            None
        }
    }
}

impl<'a> AsRef<Collections> for Snapshots<'a> {
    fn as_ref(&self) -> &Collections {
        self.backup._collections()
    }
}


impl<'a> Snapshot<'a> {
    /// Returns the time in which the snapshot has been taken.
    pub fn time(&self) -> Timespec {
        self.set.end_time()
    }

    /// Returns true if the snapshot is a full backup.
    ///
    /// A full snapshot does not depend on previous snapshots.
    pub fn is_full(&self) -> bool {
        self.set.is_full()
    }

    /// Returns true if the snapshot is an incremental backup.
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

    /// Returns the files present in the snapshot.
    ///
    /// Be aware that using this functionality means that all the signature files in the current
    /// backup chain must be loaded, and this could take some time, depending on the file access
    /// provided by the backend.
    pub fn files(&self) -> io::Result<SnapshotFiles> {
        let sig = try!(self.backup._signature_chain(self.chain_id));
        if self.sig_id < sig.as_ref().unwrap().snapshots().len() {
            Ok(SnapshotFiles {
                chain: sig,
                sig_id: self.sig_id,
            })
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "The signature chain is incomplete"))
        }
    }
}

impl<'a> AsRef<BackupSet> for Snapshot<'a> {
    fn as_ref(&self) -> &BackupSet {
        self.set
    }
}


impl<'a> SnapshotFiles<'a> {
    /// Converts the snapshot files into the signature representation.
    ///
    /// This function can be used to retrieve lower level informations about the files in the
    /// snapshot.
    pub fn as_signature_info(&self) -> signatures::SnapshotFiles {
        self.chain.as_ref().unwrap().snapshots().nth(self.sig_id).unwrap().files()
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
                    return Err(io::Error::new(io::ErrorKind::NotFound,
                                              "The given backup snapshot does not have a \
                                              corresponding signature"));
                }
            }
        }

        // need to close previous scope to borrow again
        // return the cached value
        Ok(self.signatures[chain_id].borrow())
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use backend::local::LocalBackend;
    use collections::{BackupSet, Collections};
    use signatures::{Chain, File};

    use std::path::PathBuf;

    use time::Timespec;


    #[derive(Debug, Eq, PartialEq)]
    struct SnapshotTest {
        time: Timespec,
        is_full: bool,
        num_volumes: usize,
    }

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct FileTest {
        path: PathBuf,
        mtime: Timespec,
        uname: String,
        gname: String,
    }

    impl FileTest {
        pub fn from_file(file: &File) -> Self {
            FileTest {
                path: file.path().to_owned(),
                mtime: file.mtime(),
                uname: file.username().unwrap().to_owned(),
                gname: file.groupname().unwrap().to_owned(),
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
        let filenames = backend.get_file_names().unwrap();
        let coll = Collections::from_filenames(filenames);
        Chain::from_sigchain(coll.signature_chains().next().unwrap(), &backend).unwrap()
    }

    fn from_sigchain(chain: &Chain) -> Vec<Vec<FileTest>> {
        chain.snapshots()
             .map(|s| {
                 s.files()
                  .map(|f| FileTest::from_file(&f))
                  .filter(|f| f.path.to_str().is_some())
                  .collect::<Vec<_>>()
             })
             .collect::<Vec<_>>()
    }


    #[test]
    fn same_collections() {
        let backend = LocalBackend::new("tests/backups/single_vol");
        let filenames = backend.get_file_names().unwrap();
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
        let actual = backup.snapshots()
                           .unwrap()
                           .map(|s| {
                               s.files()
                                .unwrap()
                                .as_signature_info()
                                .map(|f| FileTest::from_file(&f))
                                .filter(|f| f.path.to_str().is_some())
                                .collect::<Vec<_>>()
                           })
                           .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
