

use ::not_found;
use read::block::{BLOCK_SIZE, BlockId};
use read::cache::BlockCache;
use signatures::EntryId;
use std::borrow::Cow;
use std::io::{self, Read};
use std::str::{self, FromStr};
use std::usize;

use tar;


pub struct VolumeReader<R: Read, S: ResolveEntryPath> {
    arch: tar::Archive<R>,
    resolver: S,
}

pub trait ResolveEntryPath {
    fn resolve(&mut self, path: &[u8]) -> Option<EntryId>;
}


struct EntryInfo<'a> {
    path: Cow<'a, [u8]>,
    block_num: Option<usize>,
    etype: EntryType,
}

enum EntryType {
    Deleted,
    Diff,
    Snapshot,
}


impl<R: Read, S: ResolveEntryPath> VolumeReader<R, S> {
    pub fn new(archive: tar::Archive<R>, resolver: S) -> Self {
        VolumeReader {
            arch: archive,
            resolver: resolver,
        }
    }

    /// Cache a specific block in the volume.
    ///
    /// Include also the caching of the next `additional` blocks.
    pub fn cache_block(&mut self,
                       dcache: &BlockCache,
                       scache: &BlockCache,
                       req_block: BlockId,
                       additional: usize)
                       -> io::Result<()> {
        let mut block = vec![0u8; BLOCK_SIZE];
        let mut n_cached = 0;
        // io errors are treated as hard errors
        for entry in try!(self.arch.entries()) {
            if n_cached > additional {
                // just break if we have cached everything
                break;
            }
            let mut entry = match entry {
                Ok(entry) => entry,
                _ => {
                    // unfortunately volume tar files are not compliant, they miss the last null
                    // block, so we just return if an entry is invalid, because it's probably the
                    // last one
                    return Ok(());
                }
            };
            let (block_id, cache) = {
                let info = match EntryInfo::new(entry.path_bytes()) {
                    Some(info) => info,
                    None => {
                        continue; // skip bad block
                    }
                };
                let block_id = match self.resolver.resolve(&info.path) {
                    Some(id) => (id, info.block_num.unwrap_or(0)),
                    None => {
                        continue; // skip unknown entries
                    }
                };
                // test the block id
                if block_id < req_block {
                    // if the requested block is not arrived yet, just skip
                    continue;
                }
                if block_id > req_block && n_cached == 0 {
                    return Err(not_found("the requested block has not been found in the volume"));
                }
                let cache = match info.etype {
                    EntryType::Deleted => {
                        continue; // skip deleted entries
                    }
                    EntryType::Diff => dcache,
                    EntryType::Snapshot => scache,
                };
                (block_id, cache)
            };
            // we have found a useful entry, so cache it
            n_cached += 1;
            // insert in the cache only if not already present
            if !cache.cached(block_id) {
                let len = try!(entry.read(&mut block));
                cache.write(block_id, &block[..len]);
            }
        }
        Ok(())
    }

    pub fn cache_all(&mut self, dcache: &BlockCache, scache: &BlockCache) -> io::Result<()> {
        let mut block = vec![0u8; BLOCK_SIZE];
        // io errors are treated as hard errors
        for entry in try!(self.arch.entries()) {
            let mut entry = match entry {
                Ok(entry) => entry,
                _ => {
                    // unfortunately volume tar files are not compliant, they miss the last null
                    // block, so we just return if an entry is invalid, because it's probably the
                    // last one
                    return Ok(());
                }
            };
            let (block_id, cache) = {
                let info = match EntryInfo::new(entry.path_bytes()) {
                    Some(info) => info,
                    None => {
                        continue; // skip bad block
                    }
                };
                let cache = match info.etype {
                    EntryType::Deleted => {
                        continue; // skip deleted entries
                    }
                    EntryType::Diff => dcache,
                    EntryType::Snapshot => scache,
                };
                let block_id = match self.resolver.resolve(&info.path) {
                    Some(id) => (id, info.block_num.unwrap_or(0)),
                    None => {
                        continue; // skip unknown entries
                    }
                };
                (block_id, cache)
            };
            // insert in the cache only if not already present
            if !cache.cached(block_id) {
                let len = try!(entry.read(&mut block));
                cache.write(block_id, &block[..len]);
            }
        }
        Ok(())
    }
}


impl<'a> EntryInfo<'a> {
    pub fn new(full_path: Cow<'a, [u8]>) -> Option<Self> {
        // parse the type
        let pos = try_opt!(full_path.iter().cloned().position(|b| b == b'/')) + 1;
        let (etype, multivol) = match &full_path[..pos - 1] {
            b"diff" => (EntryType::Diff, false),
            b"deleted" => (EntryType::Deleted, false),
            b"snapshot" => (EntryType::Snapshot, false),
            b"multivol_diff" => (EntryType::Diff, true),
            b"multivol_snapshot" => (EntryType::Snapshot, true),
            _ => {
                return None;
            }
        };
        // parse the block number
        let epos = {
            if multivol {
                full_path.iter().cloned().rposition(|b| b == b'/')
            } else {
                None
            }
        };
        let vol_num = match epos {
            Some(pos) if pos + 1 < full_path.len() => {
                let bnum = &full_path[pos + 1..];
                str::from_utf8(bnum)
                    .ok()
                    .and_then(|strnum| usize::from_str(strnum).ok())
            }
            _ => None,
        };
        // extract the path
        let end_pos = match epos {
            Some(epos) if epos >= pos => epos,
            _ => full_path.len(),
        };
        let path = match full_path {
            Cow::Borrowed(fp) => Cow::Borrowed(&fp[pos..end_pos]),
            Cow::Owned(fp) => Cow::Owned(fp[pos..end_pos].to_owned()),
        };

        Some(EntryInfo {
            path: path,
            block_num: vol_num,
            etype: etype,
        })
    }
}


#[cfg(test)]
mod test {
    use flate2::read::GzDecoder;
    use read::cache::BlockCache;
    use signatures::EntryId;

    use std::fs::File;
    use super::*;
    use tar::Archive;

    const TEST_VOL: &'static str = "tests/backups/single_vol/\
        duplicity-inc.20150617T182629Z.to.20150617T182650Z.vol1.difftar.gz";

    fn test_vol_entries() -> Vec<Vec<u8>> {
        vec![b"".to_vec(),
            b"changeable_permission".to_vec(),
            b"directory_to_file".to_vec(),
            b"executable2".to_vec(),
            b"executable2/another_file".to_vec(),
            b"file_to_directory".to_vec(),
            b"largefile".to_vec(),
            b"new_file".to_vec(),
            b"regular_file".to_vec(),
            b"symbolic_link".to_vec(),
        ]
    }

    struct TestResolver {
        data: Vec<Vec<u8>>,
    }

    impl ResolveEntryPath for TestResolver {
        fn resolve(&mut self, path: &[u8]) -> Option<EntryId> {
            self.data.iter().position(|elem| elem.as_slice() == path).map(|x| (x, 0))
        }
    }

    #[test]
    fn cache_all_size() {
        let resolver = TestResolver { data: test_vol_entries() };
        let archive = {
            let file = File::open(TEST_VOL).unwrap();
            let gz_decoder = GzDecoder::new(file).unwrap();
            Archive::new(gz_decoder)
        };
        let dcache = BlockCache::new(1000);
        let scache = BlockCache::new(1000);
        let mut volread = VolumeReader::new(archive, resolver);
        volread.cache_all(&dcache, &scache).unwrap();
        assert_eq!(scache.size(), 2);
        assert_eq!(dcache.size(), 56);
    }

    #[test]
    fn cache_block() {
        let resolver = TestResolver { data: test_vol_entries() };
        let archive = {
            let file = File::open(TEST_VOL).unwrap();
            let gz_decoder = GzDecoder::new(file).unwrap();
            Archive::new(gz_decoder)
        };
        let dcache = BlockCache::new(1000);
        let scache = BlockCache::new(1000);
        let id = ((6, 0), 5); // largefile/5
        let mut volread = VolumeReader::new(archive, resolver);
        volread.cache_block(&dcache, &scache, id, 10).unwrap();
        assert_eq!(scache.size(), 0);
        assert_eq!(dcache.size(), 11);
        // requested block and 10 next are present
        for i in 0..11 {
            let id = ((6, 0), 5 + i);
            assert!(dcache.cached(id));
        }
        // previous is not
        assert!(!dcache.cached(((6, 0), 4)));
    }
}
