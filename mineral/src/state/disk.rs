
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::io::Result;
#[cfg(target_family = "unix")]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

use super::{MetaData, State};

#[derive(Debug)]
pub struct Disk {
    path: String,
    handle: File,
}

impl Disk {
    pub fn new (path: &str) -> Disk {
        Disk{
            path: path.to_string(),
            handle: OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(path).unwrap(),
        }
    }

// }

// impl State for Disk  {
    pub fn set(&mut self, pos: usize, buf: &[u8]) -> Result<()> {
        self.handle.seek(SeekFrom::Start(pos as u64))?;
        self.handle.write_all(buf)?;
        Ok(())
    }

    pub fn get(&mut self, pos: usize, buf: &mut [u8]) -> Result<usize> {
        self.handle.seek(SeekFrom::Start(pos as u64))?;
        let n = self.handle.read(buf)?;
        Ok(n)
    }

    pub fn get_from_end(&mut self, pos: i64, buf: &mut [u8]) -> Result<usize> {
        self.handle.seek(SeekFrom::End(pos))?;
        let n = self.handle.read(buf)?;
        Ok(n)
    }

    pub fn truncate(&mut self) ->Result<()> {
        self.handle.set_len(0)?;
        Ok(())
    }

    pub fn append(&mut self, buf: &[u8]) -> Result<()> {
        self.handle.seek(SeekFrom::End(0))?;
        self.handle.write_all(buf)?;
        Ok(())
    }

    pub fn prepend(&mut self, buf: &[u8]) -> Result<()> {
        self.handle.seek(SeekFrom::Start(0))?;
        self.handle.write_all(buf)?;
        Ok(())
    }

    pub fn meta(&self) -> Result<MetaData> {
        let metadata = self.handle.metadata();
        #[cfg(target_family = "windows")]
        let file_size = metadata.unwrap().file_size();
        #[cfg(target_family = "unix")]
        let file_size = metadata.unwrap().size();

        Ok(MetaData{
            size: file_size as usize,
        })
    }

    pub fn rename(&self, path: &str) -> Result<()> {
        fs::rename(self.path.as_str(), path)
    }

    pub fn remove(&self) -> Result<()> {
        fs::remove_file(self.path.as_str())
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set() {
        let mut disk = Disk::new("/tmp/wtfs_disk_test_set");

        let buffer = [2u8; 10];
        let _ = disk.set(100, &buffer);

        let metadata = disk.meta().unwrap();
        let _ = disk.remove();

        assert_eq!(metadata.size, 110);
    }

    #[test]
    fn test_get() {
        let mut disk = Disk::new("/tmp/wtfs_disk_test_get");

        let mut read_buf = [0u8; 10];
        let _ = disk.get(0, &mut read_buf);
        assert_eq!(read_buf, [0u8; 10]);

        let buffer = [2u8; 10];
        let _ = disk.set(100, &buffer);
 
        let mut read_buf = [0u8; 10];
        let _ = disk.get(100, &mut read_buf);
        let _ = disk.remove();

        assert_eq!(read_buf, buffer);
    }

    #[test]
    fn test_truncate() {
        let mut disk = Disk::new("/tmp/wtfs_disk_test_truncate");

        let _ = disk.truncate();
        let metadata = disk.meta().unwrap();

        let _ = disk.remove();

        assert_eq!(metadata.size, 0);
    }

    #[test]
    fn test_append() {
        let mut disk = Disk::new("/tmp/wtfs_disk_test_append");

        let _ = disk.set(100, &[2u8; 10]);
        let _ = disk.set(10, &[2u8; 10]);
        let _ = disk.append(&[3u8; 10]);
        
        let metadata = disk.meta().unwrap();

        let mut read_buf = [0u8;10];
        let _ = disk.get(110, &mut read_buf);

        let _ = disk.remove();

        assert_eq!(metadata.size, 120);
        assert_eq!(read_buf, [3u8;10]);
    }

    #[test]
    fn test_prepend() {
        let mut disk = Disk::new("/tmp/wtfs_disk_test_prepend");

        let _ = disk.set(100, &[2u8; 10]);
        let _ = disk.set(10, &[2u8; 10]);
        let _ = disk.prepend(&[3u8; 10]);
        
        let metadata = disk.meta().unwrap();

        let mut read_buf = [0u8;10];
        let _ = disk.get(0, &mut read_buf);

        let _ = disk.remove();

        assert_eq!(metadata.size, 110);
        assert_eq!(read_buf, [3u8;10]);
    }

}
