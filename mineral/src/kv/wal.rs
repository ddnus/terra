use std::time::{SystemTime, UNIX_EPOCH};

use crate::{error::Error, storage::wal::{Wal, WalReader}};

use super::Bytes;

pub struct KvWalEntryHeader {
    pub expires_at: u64, // 过期时间，精确到秒
    keylen: u32, // key 长度
}

pub struct KvWalEntry {
    pub key: String,
    pub val: Bytes,
    pub header: KvWalEntryHeader,
}

impl KvWalEntry {
    fn new(op: u8, key: &str, val: &Bytes, expires_at: u64) -> Self {
        KvWalEntry {
            header: KvWalEntryHeader{
                keylen: key.len() as u32,
                expires_at,
            },
            key: key.to_string(),
            val: val.clone()
        }
    }

    fn encode(&mut self) -> Bytes {
        let mut buf = self.header.expires_at.to_be_bytes().to_vec();
        buf.append(self.header.keylen.to_be_bytes().to_vec().as_mut());
        buf.append(self.key.as_bytes().to_vec().as_mut());
        buf.append(self.val.as_mut());
        buf
    }

    pub fn decode(buf: Bytes) -> Self {
        let expires_at = u64::from_be_bytes(buf[0..8].try_into().unwrap());
        let keylen = u32::from_be_bytes(buf[8..12].try_into().unwrap());
        let key_end = (keylen + 12) as usize;
        let key = String::from_utf8(buf[12..key_end].to_vec()).unwrap();
        let val = buf[key_end..].to_vec();
        KvWalEntry {
            header: KvWalEntryHeader{
                keylen: keylen,
                expires_at,
            }, key, val
        }
    }
}

const OP_SET: u8 = 1;
const OP_DEL: u8 = 2;

pub struct KvWal {
    wal: Wal,
}

impl KvWal {
    pub fn new(path: &str) -> Self {
        KvWal {
            wal: Wal::new(path)
        }
    }

    pub fn set(&mut self, key: &str, val: &Bytes, expire: u64) -> Result<u64, Error> {
        self.append(OP_SET, key, val, expire)
    }

    pub fn del(&mut self, key: &str) -> Result<u64, Error> {
        self.append(OP_DEL, key, &vec![], 0)
    }

    pub fn checkpoint(&mut self, version: u64) -> Vec<u64> {
        self.wal.checked_version(version)
    }

    pub fn reader(&mut self, min_version: u64, max_version: u64) -> Option<WalReader> {
        self.wal.reader(min_version, max_version)
    }

    fn append(&mut self, op: u8, key: &str, val: &Bytes, expire: u64) -> Result<u64, Error> {

        let mut entry = KvWalEntry::new(op, key, val, expire);
        
        self.wal.append(&entry.encode())
    }

}
