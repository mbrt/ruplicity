use std::io::Read;
use tar;

pub type VolumeReader<R> = tar::Archive<R>;
pub type VolumeReaderIter<'a, R> = tar::Entries<'a, R>;

pub struct VolumeEntry<'a, R: Read + 'a>(tar::Entry<'a, R>);

impl<'a, R: Read + 'a> VolumeEntry<'a, R> {
}
