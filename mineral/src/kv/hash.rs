
use std::num::NonZeroUsize;
use std::ops::Add;
use std::sync::{Arc, Mutex};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lru::LruCache;

use crate::{config::KvConfig, storage::serve::Serve};

use super::cbf::Cbf;
use super::slot::{SlotEntry, EXPIRE_DEL};
use super::wal::{KvWal, KvWalEntry};
use super::Slot;
use super::Bytes;

#[derive(Debug)]
pub struct HashKv {
    store: Arc<Mutex<Serve>>,
    wal: Arc<Mutex<KvWal>>,
    cbf: Arc<Mutex<Cbf>>,
    slots: u32,
    lru: LruCache<Bytes, SlotEntry>,
}

impl HashKv {
    pub fn new(conf: KvConfig) -> Self {
        let mut kv = HashKv {
            store: Arc::new(Mutex::new(Serve::new(conf.storage.clone()))),
            wal: Arc::new(Mutex::new(KvWal::new(&conf.wal_path))),
            cbf: Arc::new(Mutex::new(Cbf::new(conf.cbf_cap))),
            slots: conf.slot_qty,
            lru: LruCache::new(NonZeroUsize::new(conf.cache_cap).unwrap()),
        };
        
        kv.init_wal_logs();

        kv.run();
        kv

    }

    fn calculate_index<K: Hash>(&self, key: &K) -> usize {
        // 使用Rust的标准库中的Hash来计算key的哈希值
        let mut hasher = DefaultHasher::new();
    
        key.hash(&mut hasher);
        
        let hash_code = hasher.finish(); // 获取64位的哈希值
    
        // 将hashCode的高18位和低18位进行异或运算
        let xor_hash = (hash_code >> 18) ^ (hash_code & 0x3FFFF);
    
        // 与当前数组长度减一进行与运算，得到数组下标
        let index = xor_hash & (self.slots as u64 - 1);
    
        // 由于数组下标必须是usize类型，进行类型转换
        index as usize
    }

    pub fn set(&mut self, key: &Bytes, val: &Bytes) {
        self.setnx(key, val, None);
    }

    pub fn setnx(&mut self, key: &Bytes, val: &Bytes, expire: Option<Duration>) {
        let expires_at = if let Some(dur) = expire {
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().add(dur).as_secs()
        } else {
            0
        };
        
        // 优先写日志
        let version = self.wal.lock().unwrap().set(key, val, expires_at).unwrap();

        self._set_to_cbf(version, key, val, expires_at);

    }

    fn _set_to_cbf(&mut self, version: u64, key: &Bytes, val: &Bytes, expires_at: u64) {
        let slot_no = self.calculate_index(&key);

        let mut slot = if let Some(st) = self.cbf.lock().unwrap().get(slot_no) {
                st
            } else if let Ok(data) = self.store.lock().unwrap().get(slot_no) {
                Slot::new(slot_no, data).unwrap()
            } else {
                Slot::new(slot_no, vec![]).unwrap()
            };

        // 更新数据    
        slot.set(key, val, expires_at);

        // 刷新到cbf 写入缓冲中
        self.cbf.lock().unwrap().insert(version as usize, &slot).unwrap();

        // 更新lru
        self.lru.put(key.clone(), SlotEntry::new(val, expires_at));
    }

    pub fn get(&mut self, key: &Bytes) -> Option<Bytes> {

        let slot_no = self.calculate_index(&key);
        // 从lru中获取
        if let Some(entry) = self.lru.get(key) {
            if entry.has_expired() {
                return None;
            }
            return Some(entry.value.clone());
        }

        // 从cbf中获取
        if let Some(slot) = self.cbf.lock().unwrap().get(slot_no) {
            if let Some(entry) = slot.get(key) {
                if entry.has_expired() {
                    return None;
                }
                return Some(entry.value);
            }
        }

        // 向store获取磁盘中数据
        if let Ok(res) = self.store.lock().unwrap().get(slot_no) {
            let slot = Slot::new(slot_no, res).unwrap();
            if let Some(entry) = slot.get(key) {
                if entry.has_expired() {
                    return None;
                }
                // 将entry更新至lru
                self.lru.put(key.clone(), entry.clone());
                return Some(entry.value);
            }
        }

        None
    }

    pub fn del(&mut self, key: &Bytes) -> Option<Bytes> {

        let version = self.wal.lock().unwrap().del(key).unwrap();

        let slot_no = self.calculate_index(&key);

        // 从lru中获取
        let mut slot = if let Some(cache) = self.cbf.lock().unwrap().get(slot_no) {
                cache.clone()
            } else if let Ok(data) = self.store.lock().unwrap().get(slot_no) {
                Slot::new(slot_no, data).unwrap()
            } else {
                Slot::new(slot_no, vec![]).unwrap()
            };
        
        let old_slot_entry = slot.del(&key);

        // 更新cbf变更缓冲
        self.cbf.lock().unwrap().insert(version as usize, &slot).unwrap();

        // 更新lru
        self.lru.put(key.clone(), SlotEntry::new(&vec![], EXPIRE_DEL));
        
        match old_slot_entry {
            Some(old_entry) => Some(old_entry.value),
            None => None
        }

    }

    fn init_wal_logs(&mut self) {
        let wal_reder = self.wal.lock().unwrap().reader(0, 0);
        if wal_reder.is_none() {
            return;
        }

        for payload in wal_reder.unwrap() {
            let payload = payload.unwrap();
            let entry = KvWalEntry::decode(payload.data);
            self._set_to_cbf(payload.version, &entry.key, &entry.val, entry.header.expires_at);
        }
    }

    fn run(&self) {
        let cbf = self.cbf.clone();
        let store = self.store.clone();
        let wal = self.wal.clone();
        thread::spawn(move || {
            loop {
                if let Some((page_no, page)) = cbf.lock().unwrap().pop_first_page() {
                    // println!("page no: {}", page_no);
                    for (pos, buf) in page.entrys {
                        store.lock().unwrap().set(pos, buf).unwrap();
                    }

                    // 此处主要用来处理预写日志检查点
                    wal.lock().unwrap().checkpoint(page.max_version as u64);
                }
                thread::sleep(Duration::from_millis(5000));
            }
        });
    }

}

#[cfg(test)]
mod tests {
    use crate::config::StorageConfig;

    use super::*;

    fn get_conf() -> KvConfig {
        KvConfig {
            storage: StorageConfig{
                path: "/tmp/terra/tests/kv-data2".to_string(),
                block_size: 1024,
                page_max_cap: 1024 * 1024 * 50,
            },
            wal_path: "/tmp/terra/tests/kv-log2".to_string(),
            cache_cap: 1024 * 1024 * 50,
            cbf_cap: 1024 * 1024 * 50,
            slot_qty: 10000,
        }
    }

    #[test]
    fn test_set() {
        let mut kv = HashKv::new(get_conf());
        let key = "foo".as_bytes().to_vec();
        let val = "bar".as_bytes().to_vec();
        kv.set(&key, &val);
        assert_eq!(kv.get(&key).unwrap(), val);

        let old_val = kv.del(&key);
        assert_eq!(old_val.unwrap(), val);
        let new_val = kv.get(&key);
        assert!(new_val.is_none());

        kv.setnx(&key, &val, Some(Duration::from_secs(4)));
        assert_eq!(kv.get(&key).unwrap(), val);
        thread::sleep(Duration::from_secs(5));
        assert_eq!(kv.get(&key), None);
    }

    #[test]
    fn test_duration() {
        assert_eq!(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            SystemTime::now().checked_add(Duration::from_secs(4)).unwrap().elapsed().unwrap().as_secs(),
        );
        
    }
}