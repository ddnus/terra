use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[drive(Debug)]
pub struct HashIndex {

}

impl HashIndex {
    pub fn calculate_index<K: Hash>(key: &K, capacity: usize) -> usize {
        // 使用Rust的标准库中的Hash来计算key的哈希值
        let mut hasher = DefaultHasher::new();
    
        key.hash(&mut hasher);
        
        let hash_code = hasher.finish(); // 获取64位的哈希值
    
        // 将hashCode的高18位和低18位进行异或运算
        let xor_hash = (hash_code >> 18) ^ (hash_code & 0x3FFFF);
    
        // 与当前数组长度减一进行与运算，得到数组下标
        let index = xor_hash & (capacity as u64 - 1);
    
        // 由于数组下标必须是usize类型，进行类型转换
        index as usize
    }

    // pub fn 
}

