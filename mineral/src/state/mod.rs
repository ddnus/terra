use std::{fs, io::Result, path::Path};

mod disk;
use disk::*;

pub(crate) trait State {
    fn set(&mut self, pos: usize, buf: &[u8]) -> Result<()>;

    fn get(&mut self, pos: usize, buf: &mut [u8]) -> Result<usize>;

    fn get_from_end(&mut self, pos: i64, buf: &mut [u8]) -> Result<usize>;

    fn truncate(&mut self) ->Result<()>;

    fn append(&mut self, buf: &[u8]) -> Result<()>;

    fn prepend(&mut self, buf: &[u8]) -> Result<()>;

    fn meta(&self) -> Result<MetaData>;

    fn remove(&self) -> Result<()>;
}

pub struct MetaData {
    pub size: usize,
}

pub fn new(path: &str) -> Box<dyn State> {
    Box::new(Disk::new(path))
}

pub fn build_path(path: &str, file_name: &str) -> String {
    let binding = Path::new(path);
    if !binding.exists() {
        let _ = fs::create_dir_all(path);
    }
    let binding = binding.join(file_name);
    let datablock_path = binding.to_str().unwrap();
    datablock_path.to_string()
}