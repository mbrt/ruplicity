#![deny(missing_copy_implementations,
        trivial_casts, trivial_numeric_casts,
        unsafe_code,
        unstable_features,
        unused_import_braces, unused_qualifications)]

#![cfg_attr(feature = "unstable", allow(unstable_features))]
#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(clippy))]

#![cfg_attr(test, allow(unsafe_code))]

extern crate flate2;
extern crate regex;
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
