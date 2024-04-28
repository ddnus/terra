use lru::LruCache;
use std::num::NonZeroUsize;

use crate::error::Error;



pub struct PageCache {
    lru: LruCache<usize, Vec<u8>>,
}

impl PageCache {
    fn new(path: &str, fetch_size: usize, cap: usize) -> Self {
        PageCache {
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru() {
        use lru::LruCache;
use std::num::NonZeroUsize;
let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());

assert_eq!(None, cache.put(1, "a"));
assert_eq!(None, cache.put(2, "b"));
assert_eq!(Some("b"), cache.put(2, "beta"));

assert_eq!(cache.get(&1), Some(&"a"));
assert_eq!(cache.get(&2), Some(&"beta"));
    }

}