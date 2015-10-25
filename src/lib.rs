#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(clippy))]

extern crate chrono;
extern crate flate2;
extern crate regex;
extern crate tar;
#[macro_use]
extern crate try_opt;

mod macros;
mod time_utils;
pub mod backend;
pub mod collections;
pub mod signatures;
