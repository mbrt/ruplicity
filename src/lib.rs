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
use collections::{Collections, BackupChain};
use signatures::BackupFiles;


pub struct Backup<B> {
    backend: B,
    collections: Collections,
    signatures: RefCell<Option<BackupFiles>>,
}

pub struct Snapshots<'a> {
    set_iter: Option<BackupSetIter<'a>>,
    backup: &'a ResourceCache,
}

pub struct Snapshot<'a> {
    set: &'a collections::BackupSet,
}

pub type SnapshotFiles<'a> = signatures::SnapshotFiles<'a>;


struct BackupSetIter<'a> {
    coll_iter: collections::ChainIter<'a, BackupChain>,
    curr_chain: Option<&'a BackupChain>,
    full_returned: bool,
    chain_iter: collections::BackupSetIter<'a>,
}

trait ResourceCache {
    fn _collections(&self) -> Snapshots;
}


impl<B: Backend> Backup<B> {
    pub fn new(backend: B) -> io::Result<Self> {
        let files = try!(backend.get_file_names());
        Ok(Backup {
            backend: backend,
            collections: Collections::from_filenames(files),
            signatures: RefCell::new(None),
        })
    }

    pub fn snapshots(&self) -> io::Result<Snapshots> {
        let set_iter = {
            let mut coll_iter = self.collections.backup_chains();
            let curr_chain = coll_iter.next();
            if let Some(chain) = curr_chain {
                Some(BackupSetIter {
                    coll_iter: coll_iter,
                    curr_chain: curr_chain,
                    full_returned: false,
                    chain_iter: chain.inc_sets(),
                })
            } else {
                None
            }
        };

        Ok(Snapshots {
            set_iter: set_iter,
            backup: self,
        })
    }
}


impl<'a> Iterator for Snapshots<'a> {
    type Item = Snapshot<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut iter) = self.set_iter {
            loop {
                if let Some(chain) = iter.curr_chain {
                    // check if the full set has been already returned
                    // for the current backup chain
                    if !iter.full_returned {
                        iter.full_returned = true;
                        return Some(Snapshot { set: chain.full_set() });
                    } else {
                        if let Some(set) = iter.chain_iter.next() {
                            return Some(Snapshot { set: set });
                        }
                        // the current chain has been exausted
                        // go to the next one if present
                        iter.curr_chain = iter.coll_iter.next();
                        iter.full_returned = false;
                        if let Some(chain) = iter.curr_chain {
                            iter.chain_iter = chain.inc_sets();
                        }
                    }
                } else {
                    // last chain exausted
                    break;
                }
            }
        }
        None
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

    pub fn files(&self) -> SnapshotFiles {
        unimplemented!()
    }
}


impl<B: Backend> ResourceCache for Backup<B> {
    fn _collections(&self) -> Snapshots {
        self.snapshots().unwrap()
    }
}

