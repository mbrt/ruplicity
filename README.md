# ruplicity
[![Build Status](https://travis-ci.org/mbrt/ruplicity.svg?branch=master)](https://travis-ci.org/mbrt/ruplicity)
[![Build status](https://ci.appveyor.com/api/projects/status/opwyq9ac7ji61bpp/branch/master?svg=true)](https://ci.appveyor.com/project/mbrt/ruplicity/branch/master)
[![Coverage Status](https://coveralls.io/repos/mbrt/ruplicity/badge.svg?branch=master&service=github)](https://coveralls.io/github/mbrt/ruplicity?branch=master)
[![](http://meritbadge.herokuapp.com/ruplicity)](https://crates.io/crates/ruplicity)

Rust library to explore [duplicity backups](http://duplicity.nongnu.org/).

[API documentation](http://mbrt.github.io/ruplicity)

## Installation

Add the corresponding entry to your `Cargo.toml` dependencies:

```toml
[dependencies]
ruplicity = "0.2"
```

and add `extern crate ruplicity` to your crate root.

## Motivations

Why I chose to implement a duplicity backup reader in Rust? What are the differencies with duplicity?

1. Performances. Listing the files in a 195 GB backup from an external hard drive takes 9.1 seconds in my laptop with ruplicity and 166 seconds with duplicity with an empty cache. The time goes down to 33 seconds for duplicity by having cached the backup signatures in the hard drive. This is still a huge performance gain, however I believe that we can still improve the time a lot.
2. Provide an easy to use library to implement features such as a [command line utility](https://github.com/mbrt/ruplicity-console), and a [fuse filesystem](https://github.com/mbrt/ruplicity-fuse) to mount a backup directly in your file system (that is not easily implementable within duplicity).

This library does not aim to replace duplicity, since it does not provide actual backup / restore functionalities, and it does not have the [many backends](http://duplicity.nongnu.org/duplicity.1.html) duplicity has. However, feel free to contribute if you need them.

## Example

This example demonstrates the opening of a backup stored in a local directory, and printing the files present in each backup snapshot.

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

## Contributing

Contributions are welcome! There are lots of features still to be implemented. The most important are:

* improve the code; I need some feedback from experienced Rustaceans here :);
* improve performances (since there is always room for a boost);
* implement new features such as [read backup file contents](https://github.com/mbrt/ruplicity/issues/30), and new backends (e.g. Dropbox, Azure, FTP, etc.), like duplicity does;
* support encrypted backups; this is actually more feasible when a rust GPG library is implemented.

## License

This crate is distributed under the MIT license. See LICENSE for details.

And for those who are wondering: Can you use this license even if duplicity project is licensed under GNU GPL v2? Yes, because this project does not take a single line of code of duplicity and I wanted a permissive license to ease the use of the crate.
