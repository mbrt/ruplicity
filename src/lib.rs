#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(clippy))]

extern crate flate2;
extern crate regex;
extern crate tar;
extern crate time;
#[macro_use]
extern crate try_opt;

mod macros;
mod time_utils;
pub mod backend;
pub mod collections;
pub mod signatures;
