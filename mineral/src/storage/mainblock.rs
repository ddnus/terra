use crate::storage::{datablock::DataBlock};
use crate::state::{self, State};
use std::{cmp, io::Result};
use byteorder::{BigEndian, ByteOrder};

const MAIN_BLOCK_FILE_NAME: &str = "@mainblock";
const HEADER_SIZE: usize = 17;

pub struct MainBlock {
    path: String,
    
    state: Box<dyn State>,
    // 一次性读取数据量
    fetch_size: usize,

    datablock: DataBlock,
}

pub struct Block {
    header: Header, // 数据头部信息
    data: Vec<u8>, // 数据块
}

pub struct Header {
    flag: u8,   // 标识位
    size: u64,  // 最大支持到2^32 - 1
    pos: u64,  // 数据所在位置，此处存储位图索引位置即可
}

impl MainBlock {

    pub fn new(path: &str, fetch_size: usize) -> Self {
        let main_block_file = state::build_path(path, MAIN_BLOCK_FILE_NAME);
        MainBlock {
            path: path.to_string(),
            state: state::new(&main_block_file),
            fetch_size: fetch_size,
            datablock: DataBlock::new(path, 1024),
        }
    }

    fn get_header(&mut self, index: usize) -> Result<Header> {
        let mut buf = vec![0u8; self.fetch_size];
        
        self.state.get(self.get_real_pos(index), &mut buf)?;

        let header = self.cast_to_header(&buf);

        Ok(header)
    }

    fn get_real_pos(&self, index: usize) -> usize {
        self.fetch_size * index
    }

    pub fn get(&mut self, index: usize) -> Result<Vec<u8>> {

        let mut buf = vec![0u8; self.fetch_size];

        self.state.get(self.get_real_pos(index), &mut buf)?;

        if buf.len() == 0 {
            return Ok(vec![]);
        }

        let header = self.cast_to_header(&buf);

        let mut main_data: Vec<u8> = buf[HEADER_SIZE..].to_vec();

        // 溢出情况，需要去数据块取
        if header.flag > 0 && header.size as usize > (self.fetch_size - HEADER_SIZE) {
            let remain_size = header.size as usize + HEADER_SIZE - self.fetch_size;
            let datablock = self.datablock.get(header.pos as usize, remain_size);
            match datablock {
                Ok(data) => main_data.extend(&data),
                Err(_) => {},
            }
        } else {
            let real_size = cmp::min(self.fetch_size, header.size as usize + HEADER_SIZE);
            main_data = buf[HEADER_SIZE..real_size].to_vec();
        }

        Ok(main_data)
    }

    pub fn set(&mut self, index: usize, buf: &Vec<u8>) -> Result<()> {

        let mut header = self.get_header(index)?;

        header.flag = 0;

        let total_buf_size = buf.len();
        let main_buf_size = cmp::min(self.fetch_size - HEADER_SIZE, total_buf_size);

        let mut main_buf_data = buf[..main_buf_size].to_vec();

        // 修改和新增
        if header.flag == 1 {
            let old_data_buf_size = header.size as usize + HEADER_SIZE - self.fetch_size;
            // data buf中要存储的数据
            if total_buf_size > main_buf_size {
                header.flag = 1;
                header.pos = self.datablock.update(header.pos as usize, old_data_buf_size, &buf[main_buf_size..].to_vec())? as u64;
            } else {
                self.datablock.free(header.pos as usize, old_data_buf_size)?;
            }
        } else {
            // data buf中要存储的数据
            if total_buf_size > main_buf_size {
                header.flag = 1;
                header.pos = self.datablock.set(&buf[main_buf_size..].to_vec())? as u64;
            }
        }

        header.size = total_buf_size as u64;

        let mut block = self.cast_header_to_buf(&header);
        block.append(&mut main_buf_data);

        // 保存header信息
        self.state.set(self.get_real_pos(index), &block)?;

        Ok(())
    }

    pub fn truncate(&mut self) -> Result<()> {
        self.datablock.truncate()?;
        self.state.truncate()?;
        Ok(())
    }

    fn cast_to_header(&self, buf: &Vec<u8>) -> Header {
        Header {
            flag: buf[0],
            size: BigEndian::read_u64(&buf[1..9]),
            pos: BigEndian::read_u64(&buf[9..17]),
        }
    }

    fn cast_header_to_buf(&self, header: &Header) -> Vec<u8> {
        let mut header_buf = vec![header.flag];
        let mut size_buf = [0u8; 8];
        let mut pos_buf = [0u8; 8];

        BigEndian::write_u64(&mut size_buf, header.size);
        BigEndian::write_u64(&mut pos_buf, header.pos);

        header_buf.extend_from_slice(&size_buf);
        header_buf.extend_from_slice(&pos_buf);

        header_buf
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(path: &str) -> String {
        std::env::temp_dir().to_str().unwrap().to_string() + "/wtfs/tests/" + path
    }

    #[test]
    fn test_get() {
        let mut mb = MainBlock::new(&tmp_path("mainblock1"), 1024);
        let _ = mb.truncate();
        let get_buf = mb.get(100);
        assert!(get_buf.is_ok());
        assert!(get_buf.unwrap().is_empty());
    }

    #[test]
    fn test_get_and_set() {
        let mut mb = MainBlock::new(&tmp_path("mainblock2"), 1024);
        let _ = mb.truncate();

        let list: Vec<(u8, usize, usize)> = vec![
            (3, 1024, 0),
            (1, 103, 1),
            (2, 1007, 2),
            (3, 1008, 3),
            (1, 2048, 5),
            (6, 6048, 4),
        ];

        for item in list {
            let set_buf = vec![item.0; item.1];
            match mb.set(item.2, &set_buf) {
                Ok(()) => {
                    if let Ok(get_buf) = mb.get(item.2) {
                        assert_eq!(get_buf, set_buf);
                    } else {
                        assert!(false);
                    }
                },
                _ => assert!(false),
            }
        }

    }

}