use crc32fast::Hasher;
use glob::glob;

use crate::{cvt, flate};
use crate::error::Error;
use crate::state::{self, State};
use std::fmt::Display;
use std::sync::Mutex;
use std::vec;

// 操作标识
const OP_ADD: u8 = 1;
const OP_UPDATE: u8 = 2;
const OP_CLEAN: u8 = 3;

const HEADER_LEN: u8 = 7;

const STYPE_FULL: u8 = 4;
const STYPE_FIRST: u8 = 1;
const STYPE_MIDDLE: u8 = 2;
const STYPE_LAST: u8 = 3;


// payload
//    1      4      n        8
// +----+--------+------+---------+
// | op | offset | data | version |
// +----+--------+------+---------+
//
pub struct Payload {
    op: u8,
    offset: u32,
    data: Vec<u8>,
    version: u32,
}

impl Payload {
    fn new(op: u8, offset: u32, data: &Vec<u8>, version: u32) -> Self {
        Payload {
            op,
            offset,
            data: data.to_vec(),
            version,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.op);
        buf.extend(self.offset.to_le_bytes());
        buf.extend_from_slice(&self.data);
        buf.extend(self.version.to_le_bytes());
        buf
    }
}
// entry data struct
//    4          2       1       n      
// +-------+----------+------+---------+
// | crc32 | data_len | type | payload |
// +-------+----------+------+---------+
//  7 + n

struct Header {
    crc32: u32, // the data offet in the store file
    dlen: u16,  // data length
    stype: u8,  // storage type  
}

struct Entry {
    header: Header,
    payload: Vec<u8>,
}

impl Entry {
    fn new(stype: u8, payload: &Vec<u8>) -> Self {
        let header: Header = Header {
            crc32: Self::checksum(payload),
            dlen: payload.len() as u16,
            stype,
        };

        Entry {
            header,
            payload: payload.to_vec(),
        }
    }

    fn to_header(buf: &Vec<u8>) -> Header {
        Header {
            crc32: cvt::case_buf_to_u32(&buf[..4].to_vec()),
            dlen: cvt::case_buf_to_u16(&buf[4..6].to_vec()),
            stype: buf[6],
        }
    }

    fn decode(buf: &Vec<u8>) -> Result<Entry, Error> {
        let header = Self::to_header(&buf);
        let data_end_offset = (header.dlen + HEADER_LEN as u16) as usize;
        let entry = Entry {
            header: header,
            payload: buf[HEADER_LEN as usize..data_end_offset].to_vec(),
        };

        if Self::checksum(&entry.payload) == entry.header.crc32 {
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
        buf.extend_from_slice(&self.payload);
        buf
    }

    fn to_header_buf(&self) -> Vec<u8> {
        let mut buf = cvt::case_u32_to_buf(self.header.crc32);
        buf.extend_from_slice(&cvt::case_u16_to_buf(self.header.dlen));
        buf.push(self.header.stype);
        buf
    }

}

const WAL_NAME: &str = "@wal";

pub struct Wal {
    seq: u64,
    path: String,

    wlock: Mutex<u8>,
    max_size: u32,

    wlog: Wlog,

    log_version_list: Vec<u64>,
}

impl Wal {

    pub fn new(path: &str) -> Self {

        let mut log_version_list = Self::get_log_versions(path);

        let wlog = Wlog::new(path, log_version_list[log_version_list.len() - 1]);
        
        let mut wal = Wal {
            path: path.to_string(),
            seq: 0,

            wlock: Mutex::new(0),
            max_size: 4294967295,

            wlog,

            log_version_list: log_version_list,
        };

        wal.init_version();

        wal
    }

    pub fn append(&mut self, buf: &Vec<u8>) -> Result<(), Error> {
        let mut flate_buf = flate::compress_data(buf);
        self.wlock.lock();

        self.seq += 1;

        flate_buf.extend_from_slice(&cvt::case_u64_to_buf(self.seq));

        let mut data_len = flate_buf.len();
        if data_len + self.active_size as usize > self.max_size as usize {
            self.rotation_log();
        }



        Err(Error::AppendWalDataFailed)
    }

    fn rotation_log(&mut self) {
        // 如果文件不存在
        let new_index = self.get_new_log_index(self.seq);
        self.log_version_list.push(self.seq);
        
        self.seq = new_index;

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
        self.active_size = state_handle.meta().unwrap().size as u32;
    }

    fn build_log_name<T: Display>(&self, index: T) -> String {
        state::build_path(&self.path, 
            &format!("{}-{}", WAL_NAME, index))
    }

    fn get_log_versions(path: &str) -> Vec<u64> {
        let log_glob_path = state::build_path(path, 
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

        if list.len() == 0 {
            list.push(0);
        }

        list
    }

    fn init_version(log_version_list: Vec<u64>, active_wal: &Wlog) {
        let length = log_version_list.len();
        let mut latest_log =  log_version_list[length - 1];
        
        if length > 1 {
            if active_wal.file_size == 0 {
                latest_log = log_version_list[length - 2];
            }
        } else if length == 1 {
            if active_wal.file_size == 0 {
                return;
            }
        } else {
            return;
        }

        if let Ok(version) = active_wal.get_latest_version() {
            
        }
    }

}

struct Wlog {
    state: Box<dyn State>,
    version: u64,
    page_size: u32,
    file_size: u32,
}

impl Wlog {
    fn new(path: &str, seq: u64) -> Self {
        
        let log_file = state::build_path(path, &format!("{}-{}", WAL_NAME, seq));
        let state_handle = state::new(&log_file);
        Self {
            state: state_handle,
            version: seq,
            page_size: 32768,   // 32kb
            file_size: 0
        }
    }

    pub fn append(&mut self, buf: &Vec<u8>) -> Result<(), Error> {

        let mut flate_buf = buf.clone();

        let mut entry_bufs = vec![];

        let mut stype: u8 = 0;

        let mut active_size = self.file_size;

        let mut data_len = flate_buf.len();

        loop {
            let left_page_size = (self.page_size - active_size % self.page_size) as usize;

            if left_page_size > data_len {
                
                if stype == 0 {
                    stype = STYPE_FULL;
                } else if stype > 0 {
                    stype = STYPE_LAST;
                }

                let entry_buf = Entry::new(stype, &flate_buf).encode();
                
                entry_bufs.extend_from_slice(&entry_buf);

                active_size += data_len as u32;

                break;
                
            } else {
                if stype == 0 {
                    stype = STYPE_FIRST;
                } else if stype > 0 {
                    stype = STYPE_MIDDLE;
                }

                let entry_buf = Entry::new(stype, &flate_buf[..left_page_size].to_vec()).encode();

                entry_bufs.extend_from_slice(&entry_buf);

                flate_buf = flate_buf[left_page_size..].to_vec();

                data_len -= left_page_size as usize;

                active_size += left_page_size as u32;

            }
        }

        if let Ok(_) = self.state.append(&entry_bufs) {
            self.file_size += entry_bufs.len() as u32;
            return Ok(());
        }

        Err(Error::AppendWalDataFailed)
    }

    pub fn read_all(&mut self, f: fn(buf: Vec<u8>)) {
        let mut pos = 0;
        let mut left_buf: Vec<u8> = vec![]; 
        loop {
            let mut page_buf = vec![0u8; self.page_size as usize];
            let fetch_res = self.state.get(pos, &mut page_buf);

            match fetch_res {
                Ok(get_size) => {

                    left_buf.extend_from_slice(&page_buf[..]);

                    self.handle_page(&mut left_buf, f);

                    // is the last page
                    if get_size < self.page_size as usize {
                        break;
                    }
                },
                _ => {},
            }
            pos += self.page_size as usize;
        }
    }

    fn handle_page(&mut self, page_buf: &mut Vec<u8>, f: fn(buf: Vec<u8>)) {
        
        let mut remain_buf: Vec<u8> = page_buf.clone();

        let mut chunk_datas = vec![];
        let mut page_chunk_size: usize = 0;

        loop {
            if remain_buf.len() <= HEADER_LEN as usize {
                break;
            }

            let header = Entry::to_header(&remain_buf);

            let chunk_size = (HEADER_LEN as u16 + header.dlen) as usize;
            let chunk_data = remain_buf[HEADER_LEN as usize..chunk_size].to_vec();

            page_chunk_size += chunk_size;

            if Entry::checksum(&chunk_data) != header.crc32 {
                break;
            }

            remain_buf.drain(..chunk_size);

            chunk_datas.extend_from_slice(&chunk_data);

            if header.stype == STYPE_FULL || header.stype == STYPE_LAST {

                page_buf.drain(..page_chunk_size);

                f(chunk_datas);

                chunk_datas = vec![];

            }

        }

    }

    fn get_latest_version(&self) -> Result<u64, Error> {
        let mut version_buf = [0u8; 8];
        if let Ok(size) = self.state.get_from_end(-8, &mut version_buf) {
            if size == 8 {
                return Ok(cvt::case_buf_to_u64(&version_buf.to_vec()));
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

    fn tmp_bitmap_path(path: &str) -> String {
        std::env::temp_dir().to_str().unwrap().to_string() + "/terra/tests/" + path
    }

    #[test]
    fn test_add() {
        let mut wlog = Wal::new(&tmp_bitmap_path("wal"));
        let app_result = wlog.append(&vec![3u8; 20]);
        assert!(app_result.is_ok());

        // let _ = wlog.read_all(|data| {
        //     print!("================data================: {:?}", data);
        //     assert_eq!(data, vec![3u8; 10]);
        // });
    }
}