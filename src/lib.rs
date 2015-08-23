extern crate regex;

mod collections;

use collections::Collection;

pub struct Backup {
    collection : Collection
}

impl Backup {
    pub fn new() -> Self {
        Backup{ collection : Collection::open("") }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn it_works() {
    }
}
