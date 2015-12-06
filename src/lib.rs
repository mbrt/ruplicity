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
use collections::Collections;
use signatures::BackupFiles;


pub struct Backup<B> {
    backend: B,
    collections: RefCell<Option<Collections>>,
    signatures: RefCell<Option<BackupFiles>>,
}

pub struct Snapshots<'a, B: 'a> {
    collections: Ref<'a, Option<Collections>>,
    backup: &'a Backup<B>,
}


impl<B: Backend> Backup<B> {
    pub fn new(backend: B) -> Self {
        Backup {
            backend: backend,
            collections: RefCell::new(None),
            signatures: RefCell::new(None),
        }
    }

    pub fn snapshots(&self) -> io::Result<Snapshots<B>> {
        {
            // check if there is a cached collections value
            let mut coll = self.collections.borrow_mut();
            if coll.is_none() {
                // compute collections now
                let filenames = try!(self.backend.get_file_names());
                *coll = Some(Collections::from_filenames(filenames));
            }
        }

        // need to close previous scope to borrow again
        // return the cached value
        Ok(Snapshots{
            collections: self.collections.borrow(),
            backup: &self,
        })
    }
}


impl<'a, B> AsRef<Collections> for Snapshots<'a, B> {
    fn as_ref(&self) -> &Collections {
        self.collections.as_ref().unwrap()
    }
}
