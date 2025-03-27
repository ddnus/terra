
use std::{collections::HashMap, time::{SystemTime, UNIX_EPOCH}};
use crc32fast::Hasher;
use crate::error::Error;
use super::Bytes;

pub const EXPIRE_DEL: u64 = 1;

#[derive(Debug, Clone)]
pub struct SlotEntry {
    expires_at: u64,    // timestamp
    pub value: Vec<u8>,
}

impl SlotEntry {
    pub fn new(val: &Vec<u8>, exp: u64) -> Self {
        SlotEntry {
            value: val.clone(),
            expires_at: exp,
        }
    }

    pub fn has_expired(&self) -> bool {
        if self.expires_at > 0 {
            return SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() > self.expires_at;
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct Slot {
    pub slot_no: usize,
    pub slot_kv: HashMap<Bytes, SlotEntry>
}

impl Slot {

    pub fn new(slot_no: usize, bytes: Bytes) -> Result<Slot, Error> {
        if bytes.len() == 0 {
            return Ok(Slot {
                slot_no,
                slot_kv: HashMap::new(),
            });
        }

        let mut slot = Slot {
            slot_no,
            slot_kv: HashMap::new()
        };

        if bytes.len() > 0 {
            slot.decode_kv(&bytes);
        }

        Ok(slot)
    }
    
    pub fn get(&self, key: &Bytes) -> Option<SlotEntry> {
        self.slot_kv.get(key).map(|bytes| bytes.clone())
    }

    pub fn set(&mut self, key: &Bytes, val: &Bytes, expire: u64) -> Option<SlotEntry> {
        let entry = SlotEntry::new(val, expire);
        self.slot_kv.insert(key.to_vec(), entry)
    }

    pub fn del(&mut self, key: &Bytes) -> Option<SlotEntry> {
        self.slot_kv.remove(key)
    }

    pub fn _del_soft(&mut self, key: &Bytes) -> Option<SlotEntry> {
        let _data = self.get(key);
        if let Some(mut entry) = self.get(key) {
            entry.expires_at = EXPIRE_DEL;
            return self.slot_kv.insert(key.clone(), entry);
        }
        None
    }

    // pub fn decode_kv(&mut self, buf: &Vec<u8>) {
    //     let mut buf = buf.clone();
        
    //     let mut buf_len = buf.len();

    //     let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    //     while buf_len > 20 {

    //         let total_len = u32::from_be_bytes(buf[..4].try_into().unwrap());
    //         buf_len -= total_len as usize;

    //         let new_buf: Vec<u8> = buf.drain(..(total_len as usize)).collect();

    //         let crc32 = u32::from_be_bytes(new_buf[4..8].try_into().unwrap());
    //         let data_buf = new_buf[8..].try_into().unwrap();
    //         if crc32 != Self::checksum(&data_buf) {
    //             continue;
    //         }

    //         let expires_at = u64::from_be_bytes(new_buf[8..16].try_into().unwrap());
    //         // 数据已过期
    //         if expires_at > 0 && current_time > expires_at {
    //             continue;
    //         }

    //         let key_len = u32::from_be_bytes(new_buf[16..20].try_into().unwrap());
    //         let key_end = (20 + key_len) as usize;
    //         let key = String::from_utf8(new_buf[20..key_end].to_vec()).unwrap();
    //         let slot = SlotEntry::new(&new_buf[key_end..].to_vec(), expires_at);

    //         self.slot_kv.insert(key, slot);

    //     }
    // }

    pub fn decode_kv(&mut self, buf: &Vec<u8>) {
        let mut buf = buf.clone();
        
        let mut buf_len = buf.len();

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        while buf_len > 20 {

            let total_len = u64::from_be_bytes(buf[..8].try_into().unwrap());
            buf_len -= total_len as usize;

            let new_buf: Vec<u8> = buf.drain(..(total_len as usize)).collect();

            let expires_at = u64::from_be_bytes(new_buf[8..16].try_into().unwrap());
            // 数据已过期
            if expires_at > 0 && current_time > expires_at {
                continue;
            }

            let key_len = u32::from_be_bytes(new_buf[16..20].try_into().unwrap());
            let key_end = (20 + key_len) as usize;
            let key = new_buf[20..key_end].to_vec();
            let slot = SlotEntry::new(&new_buf[key_end..].to_vec(), expires_at);

            self.slot_kv.insert(key, slot);

        }

    }
    
    // // |----- header -----|-------------- data --------------|
    // // +----4------+---4---+-----8------+----4----+--n--+--n--+
    // // | total_len | crc32 | expires_at | key-len | key | val |
    // // +-----------+-------+------------+---------+-----+-----+
    // pub fn encode(&self) -> Result<Vec<u8>, Error> {
    //     let mut buf: Vec<u8> = vec![];
    //     for (key, mut val) in self.slot_kv.clone() {
    //         if val.has_expired() {
    //             continue;
    //         }
    //         let key_bits = key.as_bytes();
    //         let key_len = key_bits.len() as u32;
    //         let total_len = (key_len as usize + val.value.len() + 20) as u32;
    //         // 数据段长度            
    //         let mut data_buf = val.expires_at.to_be_bytes().to_vec();
    //         data_buf.append(key_len.to_be_bytes().to_vec().as_mut());
    //         data_buf.append(key.as_bytes().to_vec().as_mut());
    //         data_buf.append(val.value.as_mut());

    //         let crc32 = Self::checksum(&data_buf);

    //         // 开始组装buf
    //         buf.append(total_len.to_be_bytes().to_vec().as_mut());
    //         buf.append(crc32.to_be_bytes().to_vec().as_mut());
    //         buf.append(&mut data_buf);
    //     }

    //     Ok(buf)
    // }


    // |-- header--|-------------- data --------------|
    // +-----8-----+------8-----+----4----+--n--+--n--+
    // | total-len | expires-at | key-len | key | val |
    // +-----------+------------+---------+-----+-----+
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut buf: Vec<u8> = vec![];
        for (key, mut val) in self.slot_kv.clone() {
            if val.has_expired() {
                continue;
            }
            let key_len = key.len() as u32;
            let total_len = (key_len as usize + val.value.len() + 20) as u64;

            // 开始组装buf
            buf.append(total_len.to_be_bytes().to_vec().as_mut());
            buf.append(val.expires_at.to_be_bytes().to_vec().as_mut());
            buf.append(key_len.to_be_bytes().to_vec().as_mut());
            buf.append(key.to_vec().as_mut());
            buf.append(val.value.as_mut());
        }

        Ok(buf)
    }

    fn checksum(buf: &Vec<u8>) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(buf);
        hasher.finalize()
    }
    
}