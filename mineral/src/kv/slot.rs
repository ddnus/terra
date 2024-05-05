
use core::str;
use std::{collections::HashMap, time::{SystemTime, UNIX_EPOCH}};
use serde::{Deserialize, Serialize};
use crate::error::Error;
use super::Bytes;

pub const EXPIRE_DEL: u64 = 1;

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Slot {
    pub slot_no: usize,
    pub slot_kv: HashMap<String, SlotEntry>
}

impl Slot {

    pub fn new(slot_no: usize, bytes: Bytes) -> Result<Slot, Error> {
        if bytes.len() == 0 {
            return Ok(Slot {
                slot_no,
                slot_kv: HashMap::new(),
            });
        }

        match bincode::deserialize(&bytes) {
            Err(err) => Err(Error::SlotDecodeFailed(err.to_string())),
            Ok(res) => Ok(Slot {
                slot_no,
                slot_kv: res,
            }),
        }
    }
    
    pub fn get(&self, key: &str) -> Option<SlotEntry> {
        self.slot_kv.get(key).map(|bytes| bytes.clone())
    }

    pub fn set(&mut self, key: &str, val: &Bytes, expire: u64) -> Option<SlotEntry> {
        let entry = SlotEntry::new(val, expire);
        self.slot_kv.insert(key.to_string(), entry)
    }

    pub fn del(&mut self, key: &str) -> Option<SlotEntry> {
        self.slot_kv.remove(key)
    }

    pub fn del_soft(&mut self, key: &str) -> Option<SlotEntry> {
        let mut data = self.get(key);
        if let Some(mut entry) = self.get(key) {
            entry.expires_at = EXPIRE_DEL;
            return self.slot_kv.insert(key.to_string(), entry);
        }
        None
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        match bincode::serialize(&self.slot_kv) {
            Err(err) => Err(Error::SlotEncodeFailed(err.to_string())),
            Ok(res) => Ok(res),
        }
    }
    
}