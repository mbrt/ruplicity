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

use std::cell::RefCell;


pub struct Backup {
    collections: RefCell<Option<collections::CollectionsStatus>>,
    signatures: RefCell<Option<signatures::BackupFiles>>,
}

impl Backup {
    pub fn new() -> Self {
        Backup {
            collections: RefCell::new(None),
            signatures: RefCell::new(None),
        }
    }

    pub fn collections(&self) -> &collections::CollectionsStatus {
//        {
//            // check if there is a cached collections value
//            let mut coll = self.collections.borrow_mut();
//            if coll.is_some() {
//                return coll.as_ref().unwrap()
//            }
//            // compute collections now
//        }

        // recursive call to return the just cached value
        // need to close previous scope
        self.collections()
    }
}
