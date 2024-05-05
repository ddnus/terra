use std::collections::BTreeMap;
use std::io::Result;
use crate::state::disk::Disk;
use crate::storage::{bitmap::BitMap};
use crate::state;

const DATA_BLOCK_FILE_NAME: &str = "@datablock";

#[derive(Debug)]
pub struct DataBlock {
    // header: BlockHeader,
    state: Disk,

    bitmap: BitMap,

    block_size: usize,

    delay: bool,
    delay_bufs: BTreeMap<usize, Vec<u8>>,
}

impl DataBlock {
    pub fn new(path: &str, block_size: usize, delay: bool) -> Self {
        let datablock_path = state::build_path(path, DATA_BLOCK_FILE_NAME);

        DataBlock {
            state: Disk::new(datablock_path.as_str()),
            bitmap: BitMap::new(path, delay),
            block_size: block_size,
            delay,
            delay_bufs: BTreeMap::new(),
        }
    }

    pub fn get(&mut self, index: usize, size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size as usize];
        let pos = index * self.block_size;
        // 优先查询缓存
        if self.delay {
            if let Some(delay_buf) = self.delay_bufs.get(&pos) {
                return Ok(delay_buf.clone());
            }
        }

        self.state.get(pos, &mut buf)?;
        Ok(buf)
    }

    pub fn set(&mut self, buf: &Vec<u8>) -> Result<usize> {
        let block_nums = (buf.len() + self.block_size - 1) / self.block_size;
        // 先通过位图找到存储位置
        let index = self.bitmap.malloc(block_nums);

        let pos = index * self.block_size;
        if self.delay {
            self.delay_bufs.insert(pos, buf.clone());
        } else {
            self.state.set(pos, buf)?;
        }
        
        Ok(index)
    }

    pub fn free(&mut self, index: usize, size: usize) -> Result<()> {
        let block_nums = (size + self.block_size - 1) / self.block_size;
        self.bitmap.free(index, block_nums)?;
        Ok(())
    }

    pub fn truncate(&mut self) ->Result<()> {
        self.state.truncate()?;
        self.bitmap.truncate()?;
        Ok(())
    }

    pub fn update(&mut self, index: usize, old_size: usize, new_buf: &Vec<u8>) -> Result<usize> {
        // 判断是否需要重新分配空间
        let new_size = new_buf.len();
        let block_nums = (new_size + self.block_size - 1) / self.block_size;
        let old_block_nums = (old_size + self.block_size - 1) / self.block_size;

        let mut index = index;
        if block_nums != old_block_nums {
            self.free(index, old_size)?;
            index = self.bitmap.malloc(block_nums);
        }

        let pos = index * self.block_size;

        if self.delay {
            self.delay_bufs.insert(pos, new_buf.clone());
        } else {
            self.state.set(pos, new_buf)?;
        }

        Ok(index)
    }

    pub fn flush(&mut self, version: usize) -> Result<()> {
        if !self.delay {
            return Ok(());
        }

        for (pos, buf) in self.delay_bufs.clone() {
            self.state.set(pos, &buf)?;
        }
        
        // flush 
        self.bitmap.flush_all(version as u64)
    }

    pub fn checkpoint(&self) -> u64 {
        self.bitmap.checkpoint()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(path: &str) -> String {
        std::env::temp_dir().to_str().unwrap().to_string() + "/terra/tests/" + path
    }

    #[test]
    fn test_get() {
        let mut db = DataBlock::new(&tmp_path("datablock1"), 1024, false);
        let _ = db.truncate();

        let list: Vec<(u8, usize, usize)> = vec![
            (3, 1024, 0),
            (1, 1023, 1),
            (2, 100, 2),
            (3, 1025, 3),
            (1, 102, 5),
            (2, 1, 6),
        ];

        for item in list {
            let set_buf = vec![item.0; item.1];
            match db.set(&set_buf) {
                Ok(set_pos) => {
                    if let Ok(get_buf) = db.get(set_pos, item.1) {
                        assert_eq!(get_buf, set_buf);
                        assert_eq!(set_pos, item.2);
                    } else {
                        assert!(false);
                    }
                },
                _ => assert!(false),
            }
        }
    }

    #[test]
    fn test_free() {
        let mut db = DataBlock::new(&tmp_path("datablock2"), 1024, false);
        let _ = db.truncate();

        let list: Vec<(bool, usize, usize)> = vec![
                (false, 1025, 0),
                (true, 1025, 2),
                (false, 1025, 2),
                (true, 1024, 4),
            ];

        for item in list {
            let set_buf = vec![1u8; item.1];
            match db.set(&set_buf) {
                Ok(set_pos) => {

                    if item.0 {
                        let _ = db.free(set_pos, item.1);
                    }
                    
                    if let Ok(get_buf) = db.get(set_pos, item.1) {
                        assert_eq!(get_buf, set_buf);
                        assert_eq!(set_pos, item.2);
                    } else {
                        assert!(false);
                    }
                },
                _ => assert!(false),
            }
        }
    }

}