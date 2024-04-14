use crate::storage::{common, state::{self, State}};
use std::{collections::BTreeMap, io::Result, str};
use bincode::deserialize;
use serde::Deserialize;

const BITMAP_FILE_NAME: &str = "@bitmap";

pub struct BitMap {
    meta: BlockMeta,
    state: Box<dyn State>,
}

#[derive(Debug, Deserialize)]
struct Idle(usize, usize);

#[derive(Debug, Deserialize)]
pub struct BlockMeta {
    bits: Vec<u8>,
    idles: BTreeMap<usize, usize>,

    max: u64,
    cnt: u64,
    objects: u64,
}

impl BlockMeta {
    pub fn new(bits: Vec<u8>) -> Self {
        let mut block_meta = BlockMeta {
            bits: bits,
            idles: BTreeMap::new(),
            max: 0,
            cnt: 0,
            objects: 0,
        };
        block_meta.init_idles();
        block_meta
    }

    fn init_idles(&mut self) {
        let mut index: usize = 0;
        let mut size: usize = 0;
        let mut i = 0;
        for byte in self.bits.clone() {
            for bit_index in 0..8 {
                if self.get_bit(byte, bit_index as u8) == 0 {
                    if size == 0 {
                        index = i;
                    }
                    size += 1;
                } else {
                    if size > 0 {
                        self.add_idle(index, size);
                    }
                    size = 0;
                }
                i += 1;
            }
        }

        if size > 0 {
            self.add_idle(index, size);
        }
        
    }

    fn add_idle(&mut self, index: usize, size: usize) {
        let mut size = size;
        if let Some((next_index, next_size)) = self.idles.range(index..).next() {
            let next_index = *next_index;
            let index_end = index + size;
            if next_index <= index_end {
                size = next_index + *next_size - index;
                self.idles.remove(&next_index);
            }
        }

        let mut has_pre_neighbor = false;
        if let Some((pre_index, pre_size)) = self.idles.range(..index).next_back() {
            if *pre_index + *pre_size >= index {
                has_pre_neighbor = true;
                let new_pre_size = index + size - *pre_index;
                self.idles.insert(*pre_index, new_pre_size);
            }
        }

        if !has_pre_neighbor {
            self.idles.insert(index, size);
        }

    }

    fn find_idle(&mut self, len: usize) -> Option<usize> {
        let mut find_index: Option<usize> = None;
        let mut min_len = 0;
        for (key, val) in self.idles.iter() {
            if (min_len > *val || min_len == 0) && *val >= len {
                find_index = Some(*key)
            }
        }
        find_index
    }

    fn consumer_idle(&mut self, index: usize, size: usize) {
        if let Some(len) = self.idles.remove(&index) {
            self.add_idle(index + size, len - size);
        }
    }

    fn toggle_bit(&self, byte: u8, bit_index: u8) -> u8 {
        if bit_index > 7 {
            panic!("bit_index out of range (0-7)");
        }
        let bit_index = 7 - bit_index;
        byte ^ (1 << bit_index)
    }

    fn toggle_bit_2_1(&self, byte: u8, bit_index: u8) -> u8 {
        let new_byte = self.toggle_bit(byte, bit_index);
        if new_byte > byte {
            return new_byte;
        }
        byte
    }

    fn toggle_bit_2_0(&self, byte: u8, bit_index: u8) -> u8 {
        let new_byte = self.toggle_bit(byte, bit_index);
        if new_byte < byte {
            return new_byte;
        }
        byte
    }

    fn get_bit(&self, byte: u8, bit_index: u8) -> u8 {
        if bit_index > 7 {
            panic!("bit_index out of range (0-7)");
        }
        let bit_index = 7 - bit_index;
        (byte & (1 << bit_index)) >> bit_index
    }

    fn consumer_bit(&mut self, index: usize, size: usize) {
        for i in 0..size {
            let byte_index = (index + i) / 8;
            let bit_index = (index + i) % 8;
            self.bits[byte_index] = self.toggle_bit_2_1(self.bits[byte_index], bit_index as u8);
        }
    }

    fn malloc_bit(&mut self, size: usize) -> usize {
        let bits_len = self.bits.len();
        let mut count = 0;
        if bits_len > 0 {
            for bit_index in 0..8 {
                let toggled = self.toggle_bit_2_1(self.bits[bits_len - 1], bit_index as u8);
                if toggled > self.bits[bits_len - 1] {
                    self.bits[bits_len - 1] = toggled;
                    count += 1;
                } else {
                    count = 0;
                }
            }
        }

        let start_index = bits_len * 8 - count;
        
        let new_byte_cnt = (size - count + 7) / 8;
        if new_byte_cnt > 0 {
            for i in 0..new_byte_cnt {
                // 最后一个字节
                if (i + 1) == new_byte_cnt {
                    let tail_bit_cnt = (size - count) % 8;
                    self.bits.push(255 ^ (255 >> tail_bit_cnt));
                    if tail_bit_cnt > 0 {
                        self.add_idle(start_index + size, 8 - tail_bit_cnt);
                    }
                } else {
                    self.bits.push(255);
                }
            }
        }
        
        start_index
    }

    fn free_bit(&mut self, index: usize, size: usize) {
        let bytes_len = self.bits.len();
        for i in 0..size {
            let byte_index = (index + i) / 8;
            if byte_index >= bytes_len {
                break;
            }
            let bit_index = (index + i) % 8;
            self.bits[byte_index] = self.toggle_bit_2_0(self.bits[byte_index], bit_index as u8);
        }
    }
    
    fn free(&mut self, index: usize, size: usize) {
        self.free_bit(index, size);
        self.add_idle(index, size);
    }

    fn consumer(&mut self, index: usize, size: usize) {
        self.consumer_bit(index, size);
        self.consumer_idle(index, size);
    }

    fn truncate(&mut self) {
        self.bits = vec![];
        self.idles = BTreeMap::new();
    }

}

impl BitMap {
    // 创建一个新的位图，所有位都初始化为0，0位表示空闲，1位表示占用
    pub fn new(path: &str) -> Self {
        let bytemap_path = common::build_path(path, BITMAP_FILE_NAME);
        let mut stat = state::new(&bytemap_path);
        let sz = stat.meta().unwrap().size;
        let mut bites = vec![];

        if sz > 0 {
            bites = vec![0u8; sz as usize];
            let _ = stat.get(0, &mut bites);
        }

        let mut bitmap = BitMap {
            meta: BlockMeta::new(bites),
            state: stat,
        };
        
        bitmap
    }

    // 获取位图总位数
    pub fn len(&self) -> usize {
        self.meta.bits.len() * 8
    }

    // 找到n个连续为0的起始索引位置
    pub fn find_next_n_zeros(&self, n: usize) -> Option<usize> {
        let mut count = 0;
        for (byte_index, &byte) in self.meta.bits.iter().enumerate() {
            for bit_index in 0..8 {
                if self.meta.get_bit(byte, bit_index as u8) == 0 {
                    count += 1;
                    if count == n {
                        return Some((byte_index * 8 + bit_index + 1) as usize - n);
                    }
                } else {
                    count = 0;
                }
            }
        }
        None // 没有找到
    }

    pub fn free(&mut self, start_index: usize, n: usize) ->Result<()> {
        self.meta.free(start_index, n);
        self.flush(start_index, n)
    }

    // 分配空间方法
    pub fn malloc(&mut self, n: usize) -> usize {
        if let Some(start_index) = self.meta.find_idle(n) {
            self.meta.consumer(start_index, n);
            let _ = self.flush(start_index, n);
            return start_index;
        } else {
            let start_index = self.meta.malloc_bit(n);

            let _ = self.flush(start_index, n);

            return start_index;
        }
    }

    pub fn flush(&mut self, index: usize, size: usize) -> Result<()> {
        let bit_index = index / 8;
        let byte_len = (size + 7) / 8;
        self.state.set(bit_index, &self.meta.bits[bit_index..bit_index+byte_len])?;
        Ok(())
    }

    pub fn truncate(&mut self) -> Result<()> {
        self.meta.truncate();
        self.state.truncate()?;
        Ok(())
    }

    pub fn print(&self) {
        println!("groups: {}, bytes:{}", self.meta.bits.len(), self.len());
        for (_, &byte) in self.meta.bits.iter().enumerate() {
            println!("{:08b}", byte);
        }
        println!("idles: {:?}", self.meta.idles);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bitmap() {
        let mut bitmap = BitMap::new("/tmp/wtfs/tests/bitmap1");
        let _ = bitmap.truncate();
        
        assert_eq!(bitmap.len(), 0);
        assert_eq!(bitmap.meta.bits.len(), 0);

        let index = bitmap.malloc(100);
        bitmap.print();
        
        assert_eq!(index, 0);

        let _ = bitmap.free(3, 10);
        bitmap.print();
        let _ = bitmap.free(40, 60);
        bitmap.print();
        let _ = bitmap.free(9, 10);
        bitmap.print();

        let mut bitmap = BitMap::new("/tmp/wtfs/tests/bitmap1");
        bitmap.print();
        let index = bitmap.malloc(5);
        bitmap.print();
        assert_eq!(index, 40);

        let index = bitmap.malloc(10);
        assert_eq!(index, 45);
        assert_eq!(bitmap.len(), 104);
    }

    #[test]
    fn test_get_bit() {
        let mut bitmap = BitMap::new("/tmp/wtfs/tests/bitmap2");
        let _ = bitmap.truncate();
        assert_eq!(bitmap.meta.get_bit(0b00000001, 7), 1);
        assert_eq!(bitmap.meta.get_bit(0b00000010, 6), 1);
        assert_eq!(bitmap.meta.get_bit(0b00000100, 5), 1);
        assert_eq!(bitmap.meta.get_bit(0b10000000, 0), 1);
        assert_eq!(bitmap.meta.get_bit(0b10000000, 1), 0);
        
    }

    #[test]
    fn test_find_next_n_zeros() {
        let mut bitmap = BitMap::new("/tmp/wtfs/tests/bitmap2");
        let _ = bitmap.truncate();
        bitmap.meta.bits.push(0b11110000);   // 前4位被占用
        assert_eq!(bitmap.find_next_n_zeros(4), Some(4)); // 下一个4个连续的0位从索引4开始
    }

    #[test]
    fn test_malloc_and_free() {
        let mut bitmap = BitMap::new("/tmp/wtfs/tests/bitmap2");
        let _ = bitmap.truncate();
        let index = bitmap.malloc(4);
        assert_eq!(index, 0); // 应该从索引0开始分配
        let _ = bitmap.free(index, 4);
        assert_eq!(bitmap.meta.bits[0], 0); // 分配的4位应该被释放
    }

    #[test]
    #[should_panic(expected = "bit_index out of range (0-7)")]
    fn test_get_bit_out_of_range() {
        let mut bitmap = BitMap::new("/tmp/wtfs/tests/bitmap2");
        let _ = bitmap.truncate();
        bitmap.meta.get_bit(0b00000001, 8); // 应该触发恐慌，因为索引超出范围
    }
}