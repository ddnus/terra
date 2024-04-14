use std::sync::{Arc, Mutex};
use std::{io::Result, vec};
use std::cmp;
use byteorder::{BigEndian, ByteOrder};

use crate::storage::{common, mainblock::MainBlock, state::{self, State}};

pub struct Header {
    flag: u8,   // 标识位
    size: u64,  // 最大支持到2^32 - 1
    // next: u64,  // 下一数据块地址
}

pub struct Block {
    header: Header, // 数据头部信息
    data: Vec<u8>, // 数据块
}


struct Page {
    offset: u32,
    pages: usize,
    block_size: usize,
    total_size: usize,
    next_buf_pos: usize,
    
    buffer: Vec<Vec<u8>>,
    state: Box<dyn State>,
    mainblock: Arc<Mutex<MainBlock>>,
}

impl Page {
    fn new() -> Self {
        Page {
            buffer: vec![],
            state: state::new("/"),
            mainblock: Arc::new(Mutex::new(MainBlock::new("/", 1024))),

            pages: 0,
            block_size: 1048576,
            total_size: 0,
            offset: 0,
            next_buf_pos: 0,
        }
    }

    fn cast_to_header(&self, buf: &Vec<u8>) -> Header {
        Header {
            flag: buf[0],
            size: common::case_buf_to_u64(&buf[1..9].to_vec()),
        }
    }

    fn cast_header_to_buf(&self, flag: u8, size: usize) -> Vec<u8> {
        let mut header_buf = vec![flag];
        let mut size_buf = common::case_u64_to_buf(size as u64);

        header_buf.extend_from_slice(&size_buf);

        header_buf
    }

    fn set(&mut self, index: usize, buf: Vec<u8>) {

        let index_buf = common::case_u64_to_buf(index as u64);

        let mut buf = buf;
        buf.extend(index_buf);

        self.buffer.push(buf.clone());

        let mut head_buf = self.cast_header_to_buf(255, buf.len());

        head_buf.extend(buf);

        self.state.append(&head_buf);
    }

    fn init_page(&mut self) {
        let mut pos = 0;
        
        let mut remain_buf: Vec<u8> = vec![];

        loop {
            let mut buf = vec![0u8; self.block_size];
            let get_len = self.state.get(pos, &mut buf);

            remain_buf.extend(buf);

            if remain_buf.len() == 0 {
                break;
            }

            remain_buf = self.handle_buf(&remain_buf);

            if let Ok(0) = get_len {
                break;
            }
        }
    }

    fn handle_buf(&mut self, buf: &Vec<u8>) -> Vec<u8> {
        let flag = buf[0];
        if flag == 255 {
            let size = common::case_buf_to_u64(&buf[1..9].to_vec()) as usize;
            let buf_data_len = buf.len() - 9;
            if size > buf_data_len {
                return buf.clone();
            } else if size == buf_data_len {
                self.buffer.push(buf.clone());
                return vec![];
            } else {
                self.buffer.push(buf[9..size].to_vec());
                return self.handle_buf(&buf[size..].to_vec());
            }
        } else {
            for u in buf {
                if *u == 255 {

                } else {

                }
            }
        }
        vec![]
    }

    // fn handle_buf_b(&mut self, buf: &Vec<u8>) -> Vec<u8> {

    // }

    fn flush(&mut self) {
        for buf in self.buffer.clone() {
            let buf_len = buf.len();
            let index = common::case_buf_to_u64(&buf[buf_len - 9..].to_vec());
            self.mainblock.lock().unwrap().set(index as usize, &buf[..buf_len - 9].to_vec());
        }
    }
    
}


// impl Buffer {

    // pub fn new(path: &str, fetch_size: usize) -> Self {
    //     let main_block_file = Self::get_file_path(path.to_string(), MAIN_BLOCK_FILE_NAME);
    //     MainBlock {
    //         path: path.to_string(),
    //         state: crate::state::new(&main_block_file),
    //         fetch_size: fetch_size,
    //         datablock: DataBlock::new(path, 1024),
    //     }
    // }

    // pub fn get(&mut self, index: usize) -> Result<Vec<u8>> {

    //     let mut buf = vec![0u8; self.block_size];

    //     self.state.get(self.get_real_pos(index), &mut buf)?;

    //     if buf.len() == 0 {
    //         return Ok(vec![]);
    //     }

    //     let header = self.cast_to_header(&buf);

    //     let mut main_data: Vec<u8> = buf[HEADER_SIZE..].to_vec();

    //     // 溢出情况，需要去数据块取
    //     if header.flag > 0 && header.size as usize > (self.fetch_size - HEADER_SIZE) {
    //         let remain_size = header.size as usize + HEADER_SIZE - self.fetch_size;
    //         let datablock = self.datablock.get(header.pos as usize, remain_size);
    //         match datablock {
    //             Ok(data) => main_data.extend(&data),
    //             Err(_) => {},
    //         }
    //     } else {
    //         let real_size = cmp::min(self.fetch_size, header.size as usize + HEADER_SIZE);
    //         main_data = buf[HEADER_SIZE..real_size].to_vec();
    //     }

    //     Ok(main_data)
    // }

//     pub fn set(&mut self, index: usize, buf: &Vec<u8>) -> Result<()> {

//         let mut header = self.cast_to_header(buf[])?;

//         header.flag = 0;

//         let total_buf_size = buf.len();
//         let main_buf_size = cmp::min(self.fetch_size - HEADER_SIZE, total_buf_size);

//         let mut main_buf_data = buf[..main_buf_size].to_vec();

//         // 修改和新增
//         if header.flag == 1 {
//             let old_data_buf_size = header.size as usize + HEADER_SIZE - self.fetch_size;
//             // data buf中要存储的数据
//             if total_buf_size > main_buf_size {
//                 header.flag = 1;
//                 header.pos = self.datablock.update(header.pos as usize, old_data_buf_size, &buf[main_buf_size..].to_vec())? as u64;
//             } else {
//                 self.datablock.free(header.pos as usize, old_data_buf_size)?;
//             }
//         } else {
//             // data buf中要存储的数据
//             if total_buf_size > main_buf_size {
//                 header.flag = 1;
//                 header.pos = self.datablock.set(&buf[main_buf_size..].to_vec())? as u64;
//             }
//         }

//         header.size = total_buf_size as u64;

//         let mut block = self.cast_header_to_buf(&header);
//         block.append(&mut main_buf_data);

//         // 保存header信息
//         self.state.set(self.get_real_pos(index), &block)?;

//         Ok(())
//     }

//     pub fn truncate(&mut self) -> Result<()> {
//         self.datablock.truncate()?;
//         self.state.truncate()?;
//         Ok(())
//     }

//     fn cast_to_header(&self, buf: &Vec<u8>) -> Header {
//         Header {
//             flag: buf[0],
//             size: BigEndian::read_u64(&buf[1..9]),
//             next: BigEndian::read_u64(&buf[9..17]),
//         }
//     }

//     fn cast_header_to_buf(&self, header: &Header) -> Vec<u8> {
//         let mut header_buf = vec![header.flag];
//         let mut size_buf = [0u8; 8];
//         let mut pos_buf = [0u8; 8];

//         BigEndian::write_u64(&mut size_buf, header.size);
//         BigEndian::write_u64(&mut pos_buf, header.pos);

//         header_buf.extend_from_slice(&size_buf);
//         header_buf.extend_from_slice(&pos_buf);

//         header_buf
//     }
    
// }
