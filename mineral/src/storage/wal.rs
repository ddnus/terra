use crc32fast::Hasher;
use glob::glob;

use crate::state::disk::Disk;
use crate::error::Error;
use crate::state::{self};
use std::fmt::Display;
use std::sync::Mutex;
use std::time::SystemTime;
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
//     n       8    
// +------+---------+
// | data | version |
// +------+---------+
//
pub struct Payload {
    pub data: Vec<u8>,
    pub version: u64,
}

impl Payload {

    pub fn new(version: u64, data: &Vec<u8>) -> Self {
        Payload {
            version,
            data: data.clone(),
        }
    }

    pub fn decode(buf: &Vec<u8>) -> Self {
        let data_len = buf.len() - 8;
        let version = u64::from_be_bytes(buf[data_len..].try_into().unwrap());
        Payload {
            version,
            data: buf[..data_len].to_vec(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(self.data.as_slice());
        buf.extend_from_slice(&self.version.to_be_bytes());
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
    data: Vec<u8>,
}

impl Entry {
    fn new(stype: u8, data: &Vec<u8>) -> Self {
        let header: Header = Header {
            crc32: Self::checksum(data),
            dlen: data.len() as u16,
            stype,
        };

        Entry {
            header,
            data: data.to_vec(),
        }
    }

    fn to_header(buf: &Vec<u8>) -> Header {
        Header {
            crc32: u32::from_be_bytes(buf[..4].try_into().unwrap()),
            dlen: u16::from_be_bytes(buf[4..6].try_into().unwrap()),
            stype: buf[6],
        }
    }

    fn decode(buf: &Vec<u8>) -> Result<Entry, Error> {
        let header = Self::to_header(&buf);
        let data_end_offset = (header.dlen + HEADER_LEN as u16) as usize;
        let entry = Entry {
            header: header,
            data: buf[HEADER_LEN as usize..data_end_offset].to_vec(),
        };

        if Self::checksum(&entry.data) == entry.header.crc32 {
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
        buf
    }

    fn to_header_buf(&self) -> Vec<u8> {
        let mut buf = self.header.crc32.to_be_bytes().to_vec();
        buf.extend_from_slice(&self.header.dlen.to_be_bytes());
        buf.push(self.header.stype);
        buf
    }

}

const WAL_NAME: &str = "@wal";
const WAL_CK_NAME: &str = "@checked-wal";

#[derive(Debug)]
pub struct Wal {
    seq: u64,
    path: String,

    wlock: Mutex<u8>,
    file_max_size: u64,
    rotation_live_time: u64,
    rotation_time: SystemTime,

    wlog: Wlog,

    log_version_list: Vec<u64>,
}

impl Wal {

    pub fn new(path: &str) -> Self {

        let log_version_list = Self::get_log_versions(path);

        let mut wlog = Wlog::new(path, log_version_list[log_version_list.len() - 1]);

        let version =  Wal::init_version(path, &log_version_list, &mut wlog).unwrap();
        
        let wal = Wal {
            path: path.to_string(),
            seq: version,

            wlock: Mutex::new(0),
            file_max_size: 12624855040, // 10GB
            rotation_live_time: 1800,   // 30min
            rotation_time: SystemTime::now(),   // 30min

            wlog,

            log_version_list,
        };

        wal
    }

    pub fn append(&mut self, buf: &Vec<u8>) -> Result<u64, Error> {
        let mut flate_buf = buf.clone();
        // self.wlock.lock();

        self.seq += 1;
        
        flate_buf = Payload::new(self.seq, &flate_buf).encode();

        self.rotation_log(flate_buf.len(), false);

        let save_res = self.wlog.append(&flate_buf);
        
        if let Err(err) = save_res {
            return Err(err);
        }

        Ok(self.seq)
    }

    fn rotation_log(&mut self, buf_len: usize, force: bool) {

        if force || 
            buf_len + self.wlog.file_size as usize > self.file_max_size as usize  || 
            (self.rotation_live_time > 0 &&
            SystemTime::now().duration_since(self.rotation_time).unwrap().as_secs() > self.rotation_live_time) {

            self.log_version_list.push(self.seq);

            self.wlog.close();
            
            self.wlog = Wlog::new(&self.path, self.seq);

            self.rotation_time = SystemTime::now();
        }

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

    fn init_version(path: &str, log_version_list: &Vec<u64>, active_wlog: &mut Wlog) -> Result<u64, Error> {
        let length = log_version_list.len();
        
        if length > 1 {
            if active_wlog.file_size == 0 {
                let latest_log = log_version_list[length - 2];
                return Wlog::new(path, latest_log).get_latest_version();
            }
        } else if length == 1 {
            if active_wlog.file_size == 0 {
                return Ok(0);
            }
        }

        return active_wlog.get_latest_version();
    }

    pub fn reader(&mut self, min_version: u64, max_version: u64) -> Option<WalReader> {
        WalReader::new(&self, min_version, max_version)
    }

    pub fn checked_version(&mut self, lt_version: u64) -> Vec<u64> {
        if self.wlog.version <= lt_version {
            // 当活动日志存在数据时，强制轮转
            if self.wlog.file_size > 0 {
                self.rotation_log(0, true);
            }
        }

        let mut version_list = self.log_version_list.clone();
        version_list.pop();

        let mut checked_list = vec![];

        for version in version_list {
            if version >= lt_version {
                break;
            }

            if let Ok(_) = Wlog::new(&self.path, version).checked() {
                checked_list.push(version);
                self.log_version_list.remove(0);
            }
        }

        checked_list
    }

    fn truncate_all(&mut self) {
        for version in self.log_version_list.clone() {
            if self.wlog.version == version {
                let _ = self.wlog.delete();
            } else {
                let _ = Wlog::new(&self.path, version).delete();
            }
        }

        let new_wal = Self::new(&self.path);

        self.path = new_wal.path;
        self.seq = new_wal.seq;
        self.wlock = new_wal.wlock;
        self.file_max_size = new_wal.file_max_size;
        self.rotation_live_time = new_wal.rotation_live_time;
        self.rotation_time = new_wal.rotation_time;
        self.wlog = new_wal.wlog;
        self.log_version_list = new_wal.log_version_list;
    }

}

pub struct WalReader {
    path: String,
    wlog_version_list: Vec<u64>,
    wlog_reader: PageReader,
    wlog_version: u64,
    min_version: u64,
    max_version: u64,
}

impl WalReader {
    fn new(wal: &Wal, min_version: u64, max_version: u64) -> Option<WalReader> {
        let mut wal_reader = None;
        let wlog_version_list = wal.log_version_list.clone();
        for version in wlog_version_list {
            if version < min_version || (max_version > 0 && version > max_version) {
                continue;
            }

            let wlog = Wlog::new(&wal.path, version);

            wal_reader = Some(WalReader{
                path: wal.path.to_string(),
                wlog_version_list: wal.log_version_list.clone(),
                wlog_reader: PageReader::new(wlog),
                wlog_version: version,
                max_version,
                min_version,
            });

            break;
        }

        wal_reader
        
    }

}

impl Iterator for WalReader {
    type Item = Result<Payload, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let data = self.wlog_reader.next();
        if data.is_none() {
            for version in self.wlog_version_list.clone() {
                if version <= self.wlog_version {
                    continue;
                }

                if self.max_version > 0 && version > self.max_version {
                    return None;
                }
    
                let wlog = Wlog::new(&self.path, version);
                self.wlog_reader = PageReader::new(wlog);
                self.wlog_version = version;
                return self.next();
            }
        }

        data
    }
}

#[derive(Debug)]
struct Wlog {
    path: String,
    state: Disk,
    version: u64,
    page_size: u32,
    file_size: u32,
}

impl Wlog {
    fn new(path: &str, seq: u64) -> Self {
        
        let log_file = state::build_path(path, &format!("{}-{}", WAL_NAME, seq));
        let state_handle = Disk::new(&log_file);
        let size = state_handle.meta().unwrap().size as u32;
        Self {
            path: path.to_string(),
            state: state_handle,
            version: seq,
            page_size: 32768,   // 32kb
            file_size: size,
        }
    }

    pub fn close(&mut self) {
    }

    pub fn append(&mut self, buf: &Vec<u8>) -> Result<(), Error> {

        let mut flate_buf = buf.clone();

        let mut entry_bufs = vec![];

        let mut stype: u8 = 0;

        let mut active_size = self.file_size;

        let mut data_len = flate_buf.len();

        loop {
            let left_page_size = (self.page_size - active_size % self.page_size) as usize;
            
            let left_page_data_size = if left_page_size > HEADER_LEN as usize {
                left_page_size - HEADER_LEN as usize
            } else {
                0
            };

            if left_page_data_size > data_len {
                
                if stype == 0 {
                    stype = STYPE_FULL;
                } else if stype > 0 {
                    stype = STYPE_LAST;
                }

                let entry_buf = Entry::new(stype, &flate_buf).encode();
                entry_bufs.extend_from_slice(&entry_buf);

                active_size += (data_len + HEADER_LEN as usize) as u32;

                break;
                
            } else {
                if stype == 0 {
                    stype = STYPE_FIRST;
                } else if stype > 0 {
                    stype = STYPE_MIDDLE;
                }

                let entry_buf = Entry::new(stype, &flate_buf[..left_page_data_size].to_vec()).encode();

                entry_bufs.extend_from_slice(&entry_buf);

                flate_buf = flate_buf[left_page_data_size..].to_vec();

                data_len -= left_page_data_size as usize;

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

                    let payloads = self.handle_page(&mut left_buf);

                    for payload in payloads {
                        f(payload);
                    }

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

    fn handle_page(&mut self, page_buf: &mut Vec<u8>) -> Vec<Vec<u8>> {
        
        let mut remain_buf: Vec<u8> = page_buf.clone();

        let mut chunk_datas = vec![];
        let mut page_chunk_size: usize = 0;

        let mut payloads = vec![];

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

                payloads.push(chunk_datas);

                page_chunk_size = 0;

                chunk_datas = vec![];

            }

        }

        payloads

    }

    fn get_latest_version(&mut self) -> Result<u64, Error> {
        let mut version_buf = [0u8; 8];
        if let Ok(size) = self.state.get_from_end(-8, &mut version_buf) {
            if size == 8 {
                return Ok(u64::from_be_bytes(version_buf.try_into().unwrap()));
            } else {
                panic!("Read version size: {} error", size);
            }
        } else {
            panic!("Init version faild");
        }
    }

    fn checked(&mut self) -> Result<bool, Error> {
        let checked_path = state::build_path(&self.path, &format!("{}-{}", WAL_CK_NAME, self.version));
        if let Err(err) = self.state.rename(&checked_path) {
            return Err(Error::WalCheckedFailed(err));
        }
        Ok(true)
    }

    fn delete(&mut self) -> Result<bool, Error> {
        if let Err(err) = self.state.remove() {
            return Err(Error::WalDelFailed(err));
        }
        Ok(true)
    }

}

pub struct PageReader {
    fh: Wlog,
    offset: u64,
    left_buf: Vec<u8>,
    payload_cache: Vec<Vec<u8>>,
}

impl PageReader {
    fn new(wlog: Wlog) -> Self {
        Self {
            fh: wlog,
            offset: 0,
            left_buf: vec![],
            payload_cache: vec![],
        }
    }
}

impl Iterator for PageReader {
    type Item = Result<Payload, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        
        if self.payload_cache.len() > 0 {
            let payload = self.payload_cache[0].clone();
            self.payload_cache = self.payload_cache[1..].to_vec();
            return Some(Ok(Payload::decode(&payload)));
        }

        if self.offset < self.fh.file_size as u64 {
            // get next page
            let mut buffer = vec![0; self.fh.page_size as usize];
            let fetch_res = self.fh.state.get(self.offset as usize, &mut buffer);

            if let Ok(get_size) = fetch_res {
                self.left_buf.extend_from_slice(&buffer[..get_size]);
                self.payload_cache = self.fh.handle_page(&mut self.left_buf);
                self.offset += get_size as u64;

                return self.next();
            }
        }

        None
    }
}


#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;

    fn tmp_bitmap_path(path: &str) -> String {
        #[cfg(target_family = "unix")]
        return "/tmp/terra/tests/".to_string() + path;
        #[cfg(target_family = "windows")]
        return std::env::temp_dir().to_str().unwrap().to_string() + "/terra/tests/" + path;
    }

    #[test]
    fn test_add() {
        let mut wal = Wal::new(&tmp_bitmap_path("wal"));
        wal.truncate_all();
        wal.file_max_size = 409600;

        let mut list: Vec<(u8, usize)> = vec![];

        for i in 0..1000 {
            let secret_number = rand::thread_rng().gen_range(0..10000);
            list.push((rand::thread_rng().gen_range(0..254), secret_number))
        }

        for (meta, meta_len) in list.clone() {
            let app_result = wal.append(&vec![meta; meta_len]);
            assert!(app_result.is_ok());
        }

        println!("list:{:?}", wal.log_version_list);

        let mut read = wal.reader(0, 1000);

        let mut index = 0;
        for result in read.unwrap() {
            assert!(result.is_ok());
            let item = list[index];
            index += 1;
            let payload = result.unwrap();
            assert_eq!(payload.data, vec![item.0; item.1]);
            assert_eq!(payload.version, index as u64);
        }

    }
}
