use std::io::Result;

use self::disk::Disk;

mod disk;

pub(crate) trait State {
    fn set(&mut self, pos: u64, buf: &[u8]) -> Result<()>;

    fn get(&mut self, pos: u64, buf: &mut [u8]) -> Result<()>;

    fn truncate(&mut self) ->Result<()>;

    fn append(&mut self, buf: &[u8]) -> Result<()>;

    fn prepend(&mut self, buf: &[u8]) -> Result<()>;

    fn meta(&self) -> Result<MetaData>;

    fn remove(&self) -> Result<()>;
}

pub struct MetaData {
    pub size: u64,
}

pub fn new(path: &str) -> Box<dyn State> {
    Box::new(Disk::new(path))
}