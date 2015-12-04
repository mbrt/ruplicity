#![deny(missing_copy_implementations,
        trivial_casts, trivial_numeric_casts,
        unsafe_code,
        unstable_features,
        unused_import_braces, unused_qualifications)]

#![cfg_attr(feature = "unstable", allow(unstable_features))]
#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(clippy))]

#![cfg_attr(test, allow(unsafe_code))]

extern crate byteorder;
extern crate flate2;
extern crate regex;
extern crate tabwriter;
extern crate tar;
extern crate time;
#[macro_use]
extern crate try_opt;
#[cfg(test)]
#[macro_use]
extern crate lazy_static;

mod macros;
mod time_utils;
pub mod backend;
pub mod collections;
pub mod signatures;

use std::cell::{Ref, RefCell};
use std::io;

use backend::Backend;
use collections::Collections;
use signatures::BackupFiles;


pub struct Backup<'a, B: Backend + 'a> {
    backend: &'a B,
    collections: RefCell<Option<Collections>>,
    signatures: RefCell<Option<BackupFiles>>,
}

pub struct Snapshots<'a>(Ref<'a, Option<Collections>>);


impl<'a, B: Backend> Backup<'a, B> {
    pub fn new(backend: &'a B) -> Self {
        Backup {
            backend: backend,
            collections: RefCell::new(None),
            signatures: RefCell::new(None),
        }
    }

    pub fn collections(&self) -> io::Result<Snapshots> {
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
        Ok(Snapshots(self.collections.borrow()))
    }
}
