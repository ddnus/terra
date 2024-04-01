use crate::{datablock::DataBlock, state::State};
use std::io::Result;
use byteorder::{BigEndian, ByteOrder};

const MAIN_BlOCK_FILE_NAME: &str = "@mainblock";


pub struct MainBlock {
    path: String,
    // header: BlockHeader,
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

    pub fn get_file_path(path: String, name: &str) -> String {
        if path.ends_with("/") {
            path + name
        } else {
            path + "/" + name
        }
    }

    pub fn new(path: &str, fetch_size: usize) -> Self {
        let main_block_file = Self::get_file_path(path.to_string(), MAIN_BlOCK_FILE_NAME);
        MainBlock {
            path: path.to_string(),
            state: crate::state::new(&main_block_file),
            fetch_size: fetch_size,
            datablock: DataBlock::new(path, 1024),
        }
    }

    pub fn get(&mut self, pos: u64) -> Result<Block> {
        let mut buf = vec![0u8; self.fetch_size];
        
        self.state.get(pos, &mut buf)?;

        let header = self.get_header(&buf);

        let mut block = Block{
            header: header,
            data: buf[17..].to_vec(),
        };

        // 溢出情况，需要去数据块取
        if block.header.flag > 0 && block.header.size > 0 {
            
        }

        Ok(block)
        
    }

    fn get_header(&self, buf: &Vec<u8>) -> Header {
        Header{
            flag: buf[0],
            size: BigEndian::read_u64(&buf[1..9]),
            pos: BigEndian::read_u64(&buf[9..17]),
        }
    }

    fn change_header_to_buf(&self, header: &Header) -> Vec<u8> {
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