extern crate ruplicity;
#[macro_use]
extern crate clap;

use std::io::{self, Write};
use std::process;

use ruplicity::backend::Backend;
use ruplicity::backend::local::LocalBackend;
use ruplicity::collections::Collections;


fn main() {
    let matches = clap_app!(app =>
        (version: &crate_version!()[..])
        (author: "Michele Bertasi <@brt_device>")
        (about: "Command line client for inspecting duplicity backups")
        (@subcommand info =>
            (about: "informations about snapshots present in a backup")
            (@arg INPUT: +required "the path to the backup")
        )
        (@subcommand ls =>
            (about: "list files inside a backup snapshot")
            (@arg index: -i --index +takes_value "index of the snapshot, defaults the last when omitted")
            (@arg INPUT: +required "the path to the backup")
        )
    ).get_matches();

    if let Some(matches) = matches.subcommand_matches("info") {
        // calling unwrap is safe here, because INPUT is required
        let path = matches.value_of("INPUT").unwrap();
        let backend = LocalBackend::new(path);
        let collection = Collections::from_filenames(ordie(backend.get_file_names()));
        println!("{}", collection);
    } else if let Some(_) = matches.subcommand_matches("ls") {
        unimplemented!();
    }
}


// taken from BurntSushi/tabwriter
fn ordie<T, E: ToString>(r: Result<T, E>) -> T {
    match r {
        Ok(r) => r,
        Err(e) => {
            let _ = write!(&mut io::stderr(), "{}", e.to_string());
            process::exit(1);
        }
    }
}
