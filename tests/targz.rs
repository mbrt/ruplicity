extern crate flate2;
extern crate tar;

use std::fs::File;
use flate2::read::GzDecoder;
use tar::Archive;

#[test]
fn targz() {
    let file = File::open("tests/simple.tar.gz").unwrap();
    let gz_decoder = GzDecoder::new(file).unwrap();
    let mut tar = Archive::new(gz_decoder);
    let expected = ["a", "b", "c/", "c/d"];
    let actual: Vec<_> = tar.files_mut().unwrap()
        .map(|f| f.unwrap().header().path().unwrap().to_str().unwrap().to_owned())
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn single_vol_names() {
    let file = File::open("tests/backups/single_vol/duplicity-full.20150617T182545Z.vol1.difftar.gz").unwrap();
    let gz_decoder = GzDecoder::new(file).unwrap();
    let mut tar = Archive::new(gz_decoder);
    for file in tar.files_mut().unwrap() {
        if let Ok(f) = file {
            println!("{:?} {:?}", f.header().mode(), f.header().path().unwrap())
        }
    }
}

#[test]
fn link() {
    use std::io::Read;

    let file = File::open("tests/link.tar").unwrap();
    let tar = Archive::new(file);
    let contents: Vec<_> = {
        tar.files()
           .unwrap()
           .map(|f| {
               let mut s = String::new();
               f.unwrap().read_to_string(&mut s).unwrap();
               s
           })
           .collect()
    };
    let expected = vec!["file", "file_contents"];
    assert_eq!(contents, expected);
}
