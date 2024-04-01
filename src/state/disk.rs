
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::io::Result;
use std::os::unix::fs::MetadataExt;

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
}
impl State for Disk  {
    fn set(&mut self, pos: u64, buf: &[u8]) -> Result<()> {
        self.handle.seek(SeekFrom::Start(pos))?;
        self.handle.write_all(buf)?;
        Ok(())
    }

    fn get(&mut self, pos: u64, buf: &mut [u8]) -> Result<()> {
        self.handle.seek(SeekFrom::Start(pos))?;
        self.handle.read_exact(buf)?;
        Ok(())
    }

    fn truncate(&mut self) ->Result<()> {
        self.handle.set_len(0)?;
        Ok(())
    }

    fn append(&mut self, buf: &[u8]) -> Result<()> {
        self.handle.seek(SeekFrom::End(0))?;
        self.handle.write_all(buf)?;
        Ok(())
    }

    fn prepend(&mut self, buf: &[u8]) -> Result<()> {
        self.handle.seek(SeekFrom::Start(0))?;
        self.handle.write_all(buf)?;
        Ok(())
    }

    fn meta(&self) -> Result<MetaData> {
        let metadata = self.handle.metadata();
        Ok(MetaData{
            size: metadata.unwrap().size(),
        })
    }

    fn remove(&self) -> Result<()> {
        fs::remove_file(self.path.as_str())?;
        Ok(())
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
