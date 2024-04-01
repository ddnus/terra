use std::path::Path;
use std::io::Result;

use crate::{bytemap::BitMap, state::State};

const BITMAP_FILE_NAME: &str = "@bitmap";
const DATA_BlOCK_FILE_NAME: &str = "@datablock";

pub struct DataBlock {
    // header: BlockHeader,
    state: Box<dyn State>,

    bitmap: BitMap,

    block_size: u64,
}


impl DataBlock {
    pub fn new(path: &str, block_size: u64) -> Self {
        let binding = Path::new(path).join(DATA_BlOCK_FILE_NAME);
        let datablock_path = binding.to_str().unwrap();
        let binding = Path::new(path).join(BITMAP_FILE_NAME);
        let bytemap_path = binding.to_str().unwrap();

        DataBlock {
            state: crate::state::new(datablock_path),
            bitmap: BitMap::new(bytemap_path),
            block_size: block_size,
        }
    }

    pub fn get(&mut self, pos: u64, size: u64) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size as usize];
        let pos = pos * 1024;
        self.state.get(pos, &mut buf)?;
        Ok(buf)
    }

}