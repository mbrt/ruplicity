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
#[macro_use] extern crate try_opt;

mod macros;
mod time_utils;
pub mod backend;
pub mod collections;
pub mod signatures;

use std::cell::RefCell;
use std::io;

use time::Timespec;

use backend::Backend;
use collections::{BackupChain, BackupSet, Collections};
use signatures::Chain;


pub struct Backup<B> {
    backend: B,
    collections: Collections,
    signatures: Vec<RefCell<Option<Chain>>>,
}

pub struct Snapshots<'a> {
    set_iter: CollectionsIter<'a>,
    backup: &'a ResourceCache,
}

pub struct Snapshot<'a> {
    set: &'a BackupSet,
}

pub type SnapshotFiles<'a> = signatures::SnapshotFiles<'a>;


struct CollectionsIter<'a> {
    chain_iter: collections::ChainIter<'a, BackupChain>,
    incset_iter: Option<collections::BackupSetIter<'a>>,
}

trait ResourceCache {
    fn _collections(&self) -> &Collections;
}


impl<B: Backend> Backup<B> {
    pub fn new(backend: B) -> io::Result<Self> {
        let files = try!(backend.get_file_names());
        let collections = Collections::from_filenames(files);
        let signatures = {
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
                return Some(Snapshot { set: inc_set });
            }
        }
        // the current incset is exausted or not present,
        // we need to advance the chain and return the next full set if present,
        // otherwise the job is finished
        if let Some(chain) = self.set_iter.chain_iter.next() {
            self.set_iter.incset_iter = Some(chain.inc_sets());
            Some(Snapshot{ set: chain.full_set() })
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

    pub fn files(&self) -> SnapshotFiles {
        unimplemented!()
    }
}

impl<'a> AsRef<BackupSet> for Snapshot<'a> {
    fn as_ref(&self) -> &BackupSet {
        self.set
    }
}


impl<B: Backend> ResourceCache for Backup<B> {
    fn _collections(&self) -> &Collections {
        &self.collections
    }
}

