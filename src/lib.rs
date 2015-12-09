#![deny(missing_copy_implementations,
        trivial_casts, trivial_numeric_casts,
        unsafe_code,
        unstable_features,
        unused_import_braces, unused_qualifications)]

#![cfg_attr(feature = "unstable", allow(unstable_features))]
#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(clippy))]

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

use backend::Backend;
use collections::{BackupChain, BackupSet, Collections};
use signatures::Chain;


#[derive(Debug)]
pub struct Backup<B> {
    backend: B,
    collections: Collections,
    signatures: Vec<RefCell<Option<Chain>>>,
}

pub struct Snapshots<'a> {
    set_iter: CollectionsIter<'a>,
    chain_id: usize,
    sig_id: usize,
    backup: &'a ResourceCache,
}

pub struct Snapshot<'a> {
    set: &'a BackupSet,
    // the number of the parent backup chain, starting from zero
    chain_id: usize,
    sig_id: usize,
    backup: &'a ResourceCache,
}

pub struct SnapshotFiles<'a> {
    chain: Ref<'a, Option<Chain>>,
    sig_id: usize,
}


struct CollectionsIter<'a> {
    chain_iter: collections::ChainIter<'a, BackupChain>,
    incset_iter: Option<collections::BackupSetIter<'a>>,
}

trait ResourceCache {
    fn _collections(&self) -> &Collections;
    fn _signature_chain(&self, chain_id: usize) -> io::Result<Ref<Option<Chain>>>;
}


impl<B: Backend> Backup<B> {
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

    pub fn snapshots(&self) -> Snapshots {
        let set_iter = CollectionsIter {
            chain_iter: self.collections.backup_chains(),
            incset_iter: None,
        };
        Snapshots {
            set_iter: set_iter,
            chain_id: 0,
            sig_id: 0,
            backup: self,
        }
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
    pub fn time(&self) -> Timespec {
        self.set.end_time()
    }

    pub fn is_full(&self) -> bool {
        self.set.is_full()
    }

    pub fn is_incremental(&self) -> bool {
        self.set.is_incremental()
    }

    pub fn num_volumes(&self) -> usize {
        self.set.num_volumes()
    }

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
    use backend::Backend;
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
