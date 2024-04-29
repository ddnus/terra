use std::num::NonZeroUsize;

use lru::LruCache;

use crate::error::Error;

pub struct Cache {
    lru: LruCache<usize, Vec<u8>>,
}

impl Cache {
    fn new(cap: usize) -> Self {
        Cache {
            lru: LruCache::new(NonZeroUsize::new(cap).unwrap()),
        }
    }

    pub fn get(&mut self, index: usize) -> Result<Vec<u8>, Error> {
        // cache.get(&"apple").is_none()
        let cache = self.lru.get(&index);
        if cache.is_some() {
            return Ok(cache.unwrap().clone());
        }

        Ok(vec![])
    }

    pub fn set(&mut self, index: usize, buf: &Vec<u8>) -> Result<bool, Error> {
        self.lru.put(index, buf.clone());
        Ok(true)
    }
}
