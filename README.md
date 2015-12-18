# ruplicity
[![Build Status](https://travis-ci.org/mbrt/ruplicity.svg?branch=master)](https://travis-ci.org/mbrt/ruplicity)
[![Build status](https://ci.appveyor.com/api/projects/status/opwyq9ac7ji61bpp?svg=true)](https://ci.appveyor.com/project/mbrt/ruplicity)
[![Coverage Status](https://coveralls.io/repos/mbrt/ruplicity/badge.svg?branch=master&service=github)](https://coveralls.io/github/mbrt/ruplicity?branch=master)

[API documentation](http://mbrt.github.io/ruplicity)

Rust library managing duplicity backups.

## Installation

Add the corresponding entry to your `Cargo.toml` dependencies:

```
[dependencies]
ruplicity = "0.2"
```

and add `extern crate ruplicity` to your crate root.

## Motivations

Why I chose to implement a duplicity backup reader in Rust? What are the differencies with duplicity?

## Example

This example demonstrates the opening of a backup in a local directory, and printing the files in each backup snapshot.

```rust
extern crate ruplicity;

use ruplicity::Backup;
use ruplicity::backend::local::LocalBackend;
use ruplicity::time_utils::TimeDisplay;

fn main() {
    // use the local backend to open a path in the file system containing a backup
    let backend = LocalBackend::new("tests/backups/single_vol");
    let backup = Backup::new(backend).unwrap();
    for snapshot in backup.snapshots().unwrap() {
        println!("Snapshot {}", snapshot.time().into_local_display());
        println!("{}", snapshot.entries().unwrap());
    }
}
```

Check out the documentation for advanced usages and examples.

## License

This crate is distributed under the MIT license. See LICENSE for details.

And for those who are wondering: Can you use this license even if duplicity project is licensed under GNU GPL v2? Yes, because this project does not take a single line of code of duplicity and I wanted a permissive license to ease the use of the crate.
