use std::path::Path;
use std::io::Result;

use crate::{bytemap::BitMap, state::State};

const BITMAP_FILE_NAME: &str = "@bitmap";
const DATA_BLOCK_FILE_NAME: &str = "@datablock";

pub struct DataBlock {
    // header: BlockHeader,
    state: Box<dyn State>,

    bitmap: BitMap,

    block_size: usize,
}


impl DataBlock {
    pub fn new(path: &str, block_size: usize) -> Self {
        let binding = Path::new(path).join(DATA_BLOCK_FILE_NAME);
        let datablock_path = binding.to_str().unwrap();
        let binding = Path::new(path).join(BITMAP_FILE_NAME);
        let bytemap_path = binding.to_str().unwrap();

        DataBlock {
            state: crate::state::new(datablock_path),
            bitmap: BitMap::new(bytemap_path),
            block_size: block_size,
        }
    }

    pub fn get(&mut self, pos: usize, size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size as usize];
        let pos = pos * self.block_size;
        self.state.get(pos, &mut buf)?;
        Ok(buf)
    }

    pub fn set(&mut self, buf: &Vec<u8>) -> Result<usize> {
        let block_nums = (buf.len() + self.block_size - 1) / self.block_size;
        // 先通过位图找到存储位置
        let index = self.bitmap.malloc(block_nums);

        let pos = index * self.block_size;

        self.state.set(pos, buf)?;
        
        Ok(index)
    }

    pub fn free(&mut self, pos: usize, size: usize) -> Result<()> {
        let block_nums = (size + self.block_size - 1) / self.block_size;
        self.bitmap.free(pos, block_nums)?;
        Ok(())
    }

    pub fn truncate(&mut self) ->Result<()> {
        self.state.truncate()?;
        self.bitmap.reset()?;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get() {
        let mut db = DataBlock::new("/tmp/", 1024);
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
        let mut db = DataBlock::new("/tmp/", 1024);
        let _ = db.truncate();

        let list: Vec<(bool, usize, usize)> = vec![
                (false, 1025, 0),
                (true, 1025, 2),
                (false, 1025, 2),
                (true, 1024, 4),
            ];

        for item in list {
            let set_buf = vec![0u8; item.1];
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