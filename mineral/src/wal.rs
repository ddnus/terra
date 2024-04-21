use crc32fast::Hasher;
use glob::glob;

use crate::{cvt, flate};
use crate::error::Error;
use crate::state::{self, State};
use core::panic;
use std::fmt::Display;
use std::sync::Mutex;

// 操作标识
const OP_ADD: u8 = 1;
const OP_UPDATE: u8 = 2;
const OP_CLEAN: u8 = 3;

const HEADER_LEN: u8 = 9;

// log status
type Status = char;
const STATUS_BUILD: Status = 'c';
const STATUS_ROLL: Status = 'r';
const STATUS_INVALID: Status = 'i';

struct Header {
    op: u8,
    dlen: u32,  // data length
    offset: u32,    // the data offet in the store file
}

struct Entry {
    header: Header,
    data: Vec<u8>,
    crc32: u32,
    v: u64, // version
}

impl Entry {
    fn new(v: u64, op: u8, offset: u32, buf: &Vec<u8>) -> Self {
        let header: Header = Header {
            op,
            dlen: buf.len() as u32,
            offset
        };

        Entry {
            header,
            data: buf.to_vec(),
            crc32: Self::checksum(buf),
            v,
        }
    }

    fn to_header(buf: &Vec<u8>) -> Header {
        Header {
            op: buf[0],
            dlen: cvt::case_buf_to_u32(&buf[1..5].to_vec()),
            offset: cvt::case_buf_to_u32(&buf[5..9].to_vec()),
        }
    }

    fn decode(buf: Vec<u8>) -> Result<Entry, Error> {
        let header = Self::to_header(&buf);

        let data_end_offset = (HEADER_LEN as u32 + header.dlen) as usize;
        let crc32_end_offet = data_end_offset + 4;
        let version_end_offset = crc32_end_offet + 8;
        let entry = Entry {
            header: header,
            data: buf[HEADER_LEN as usize..data_end_offset].to_vec(),
            crc32: cvt::case_buf_to_u32(&buf[data_end_offset..crc32_end_offet].to_vec()),
            v: cvt::case_buf_to_u64(&buf[crc32_end_offet..version_end_offset].to_vec()),
        };

        if Self::checksum(&entry.data) == entry.crc32 {
            return Err(Error::InvalidWalData);
        }
        
        Ok(entry)
    }

    fn checksum(buf: &Vec<u8>) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(buf);
        hasher.finalize()
    }

    fn encode(&self) -> Vec<u8> {
        let mut buf = self.to_header_buf();

        buf.extend_from_slice(&self.data);
        buf.extend_from_slice(&cvt::case_u32_to_buf(self.crc32));
        buf
    }

    fn to_header_buf(&self) -> Vec<u8> {
        let mut buf = vec![self.header.op];
        buf.extend_from_slice(&cvt::case_u32_to_buf(self.header.dlen));
        buf.extend_from_slice(&cvt::case_u32_to_buf(self.header.offset));
        buf
    }

}

const WAL_NAME: &str = "@wal";

pub struct WLog {
    state: Option<Box<dyn State>>,
    seq: u64,
    path: String,

    wlock: Mutex<u8>,
    max_size: u32,

    current_size: u32,
    current_log_version: u64, // max 255 files

    log_version_list: Vec<u64>,
}

impl WLog {

    pub fn new(path: &str) -> Self {
        let mut wlog = WLog {
            path: path.to_string(),
            state: None,
            seq: 0,

            wlock: Mutex::new(0),
            max_size: 4294967295,

            current_size: 0,
            current_log_version: 0,
            log_version_list: vec![],
        };

        wlog.log_version_list = wlog.get_log_versions();

        if wlog.log_version_list.len() == 0 {
            wlog.log_version_list.push(wlog.current_log_version);
        } else {
            // init version
            wlog.current_log_version = wlog.log_version_list[wlog.log_version_list.len() - 1];
        }

        wlog.init_log_state();

        wlog.init_version();

        wlog
    }
    // entry data struct
    // +--------+-------+-------+---------+
    // |   9    |  vlen |   4   |    8    |
    // +--------+-------+-------+---------+
    // | header | value | crc32 | version |
    // +--------+-------+-------+---------+
    pub fn append(&mut self, op: u8, offset: u32, buf: &Vec<u8>) -> Result<(), Error> {
        let flate_buf = flate::compress_data(buf);
        self.wlock.lock();

        let len = flate_buf.len();
        if len + self.current_size as usize > self.max_size as usize {
            self.rotation_log();
        }
        
        self.seq += 1;
        let entry_buf = Entry::new(self.seq, op, offset, &flate_buf).encode();

        if let Ok(_) = self.state.as_mut().unwrap().append(&entry_buf) {
            return Ok(());
        }

        Err(Error::AppendWalDataFailed)
    }

    pub fn foreach(&mut self, f: fn(buf: Vec<u8>)) {
        let batch_size = 1024 * 1024 * 5;
        
        let mut pos = 0;
        loop {
            let mut get_buf = vec![0u8; batch_size];
            let fetch_res = self.state.as_mut().unwrap().get(pos, &mut get_buf);

            match fetch_res {
                Ok(get_size) => {

                    let data_len = cvt::case_buf_to_u32(&get_buf[1..5].to_vec());


                    if get_size < batch_size {
                        break;
                    }
                },
                _ => {},
            }
            pos += batch_size;
        }
    }


    fn init_page(&mut self, f: fn(buf: Vec<u8>)) {
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
            let size = cvt::case_buf_to_u64(&buf[1..9].to_vec()) as usize;
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


    fn rotation_log(&mut self) {
        // 如果文件不存在
        let new_index = self.get_new_log_index(self.current_log_version);
        self.log_version_list.push(self.current_log_version);
        
        self.current_log_version = new_index;

        self.init_log_state();

    }

    fn get_new_log_index(&self, index: u64) -> u64 {
        let new_index = index.wrapping_add(1);
        if self.log_version_list.contains(&new_index) {
            return self.get_new_log_index(new_index);
        }
        return new_index;
    }

    fn init_log_state(&mut self) {
        let log_file = self.build_log_name(self.seq);
        let state_handle = state::new(&log_file);
        self.state = Some(state::new(&log_file));
        self.current_size = state_handle.meta().unwrap().size as u32;
    }

    fn build_log_name<T: Display>(&self, index: T) -> String {
        state::build_path(&self.path, 
            &format!("{}-{}", WAL_NAME, index))
    }

    fn get_log_versions(&self) -> Vec<u64> {
        let log_glob_path = state::build_path(&self.path, 
            &format!("{}-*", WAL_NAME));

        let globs = glob(&log_glob_path)
                .expect("Failed to read glob pattern");

        let mut list = vec![];
        for entry in globs {
            match entry {
                Ok(path) => {
                    let index_opt = path.file_name()
                        .and_then(|s| s.to_str())
                        .and_then(|s| s.split('-').last())
                        .and_then(|last| last.parse::<u64>().ok());
                    if let Some(idx) = index_opt {
                        list.push(idx);
                    }
                },
                Err(e) => println!("{:?}", e),
            }
        }

        list.sort();
        list
    }

    fn init_version(&mut self) {
        let length = self.log_version_list.len();
        let mut log_version_name =  self.log_version_list[length - 1];
        
        if length > 1 {
            if self.current_size == 0 {
                log_version_name = self.log_version_list[length - 2];
            }
        } else if length == 1 {
            if self.current_size == 0 {
                return;
            }
        } else {
            panic!("Invalid log files");
        }

        let log_file = self.build_log_name(log_version_name);
        let mut state_handle = state::new(&log_file);
        let mut version_buf = [0u8; 8];
        if let Ok(size) = state_handle.get_from_end(-8, &mut version_buf) {
            if size == 8 {
                self.seq = cvt::case_buf_to_u64(&version_buf.to_vec());
            } else {
                panic!("Read version size: {} error", size);
            }
        } else {
            panic!("Init version faild");
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let mut wlog = WLog::new("/tmp/terra");
        wlog.append(OP_ADD, 1, &vec![2u8; 10]);
    }
}