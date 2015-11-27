extern crate ruplicity;
#[macro_use]
extern crate clap;


fn main() {
    let _matches = clap_app!(app =>
        (version: &crate_version!()[..])
        (author: "Michele Bertasi <@brt_device>")
        (about: "Command line client for inspecting duplicity backups")
    ).get_matches();
}
