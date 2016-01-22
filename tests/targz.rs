extern crate flate2;
extern crate tar;

use std::fs::File;
use flate2::read::GzDecoder;
use tar::Archive;


macro_rules! unwrap_or_continue(
    ($e:expr) => (
        match $e {
            Ok(v) => v,
            _ => { continue; }
        }
    )
);

#[test]
fn targz() {
    let file = File::open("tests/simple.tar.gz").unwrap();
    let gz_decoder = GzDecoder::new(file).unwrap();
    let mut tar = Archive::new(gz_decoder);
    let expected = ["a", "b", "c/", "c/d"];
    let actual = tar.entries()
                    .unwrap()
                    .map(|f| f.unwrap().header().path().unwrap().to_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore]
fn infinite_loop() {
    let file =
        File::open("tests/backups/single_vol/duplicity-full.20150617T182545Z.vol1.difftar.gz")
            .unwrap();
    let gz_decoder = GzDecoder::new(file).unwrap();
    let mut tar = Archive::new(gz_decoder);
    for _entry in tar.entries().unwrap() {
    }
}

#[test]
#[ignore]
fn single_vol_names() {
    let file =
        File::open("tests/backups/single_vol/duplicity-full.20150617T182545Z.vol1.difftar.gz")
            .unwrap();
    let gz_decoder = GzDecoder::new(file).unwrap();
    let mut tar = Archive::new(gz_decoder);
    for file in tar.entries().unwrap() {
        if let Ok(f) = file {
            println!("{:?} {:?}",
                     f.header().mode(),
                     unwrap_or_continue!(f.header().path()))
        }
    }
}

#[test]
fn link() {
    use std::io::Read;

    let file = File::open("tests/link.tar").unwrap();
    let mut tar = Archive::new(file);
    let contents: Vec<_> = {
        tar.entries()
           .unwrap()
           .map(|f| {
               let mut f = f.unwrap();
               if f.header().entry_type().is_symlink() {
                   let link_name = f.header().link_name().unwrap().unwrap();
                   link_name.to_str().unwrap().to_owned()
               } else {
                   let mut s = String::new();
                   f.read_to_string(&mut s).unwrap();
                   s
               }
           })
           .collect()
    };
    let expected = vec!["file", "file_contents\n"];
    assert_eq!(contents, expected);
}


#[test]
fn long_path() {
    let file = File::open("tests/long_path.tar").unwrap();
    let mut tar = Archive::new(file);
    let last_entry = tar.entries().unwrap().last().unwrap().unwrap();
    let path = last_entry.path().unwrap();
    assert_eq!(path.to_str().unwrap(),
               "home/michele/Documenti/Development/Progetti/MetaCloudExperiment\
                   /Reference/duplicati/BuildTools/WixIncludeMake/Program.cs");
}
