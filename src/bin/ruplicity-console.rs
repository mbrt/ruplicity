extern crate ruplicity;
#[macro_use]
extern crate clap;

use std::io::{self, Write};
use std::process;

use ruplicity::backend::Backend;
use ruplicity::backend::local::LocalBackend;
use ruplicity::collections::Collections;
use ruplicity::signatures::BackupFiles;


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
    } else if let Some(matches) = matches.subcommand_matches("ls") {
        // calling unwrap is safe here, because INPUT is required
        let path = matches.value_of("INPUT").unwrap();
        let backend = LocalBackend::new(path);
        let backup = ordie(BackupFiles::new(&backend));
        let snapshot = {
            if let Ok(index) = value_t!(matches.value_of("index"), usize) {
                backup.snapshots().nth(index)
            } else {
                backup.snapshots().last()
            }
        };
        match snapshot {
            Some(snapshot) => {
                println!("{}", snapshot.files().into_display());
            }
            None => {
                let _ = write!(&mut io::stderr(), "Cannot find the desired snapshot in the backup\n");
                process::exit(1);
            }
        }
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
