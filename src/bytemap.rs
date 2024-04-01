use crate::state::State;
use std::io::Result;

pub struct BitMap {
    bits: Vec<u8>,
    state: Box<dyn State>,
}

impl BitMap {
    // 创建一个新的位图，所有位都初始化为0，0位表示空闲，1位表示占用
    pub fn new(path: &str) -> Self {
        let stat = crate::state::new(path);
        let sz = stat.meta().unwrap().size;

        let mut bit_map = BitMap {
            bits: vec![], // 向上取整到最近的字节
            state: stat,
        };

        if sz > 0 {
            let mut buf = vec![0u8; sz as usize];
            let _ = bit_map.state.get(0, &mut buf);
            bit_map.bits = buf.to_vec();
        } else {
            let _ = bit_map.flush();
        }
        
        bit_map
    }

    // 获取位图总位数
    pub fn len(&self) -> usize {
        self.bits.len() * 8
    }

    // 找到n个连续为0的起始索引位置
    pub fn find_next_n_zeros(&self, n: usize) -> Option<usize> {
        let mut count = 0;
        for (byte_index, &byte) in self.bits.iter().enumerate() {
            for bit_index in 0..8 {
                if self.get_bit(byte, bit_index as u8) == 0 {
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

    fn get_bit(&self, byte: u8, bit_index: u8) -> u8 {
        if bit_index > 7 {
            panic!("bit_index out of range (0-7)");
        }
        let bit_index = 7 - bit_index;
        (byte & (1 << bit_index)) >> bit_index
    }

    fn toggle_bit(&self, byte: u8, bit_index: u8) -> u8 {
        if bit_index > 7 {
            panic!("bit_index out of range (0-7)");
        }
        let bit_index = 7 - bit_index;
        byte ^ (1 << bit_index)
    }

    pub fn free(&mut self, start_index: usize, n: usize) ->Result<()> {
        for i in 0..n {
            let byte_index = (start_index + i) / 8;
            let bit_index = (start_index + i) % 8;
            self.bits[byte_index] = self.toggle_bit(self.bits[byte_index], bit_index as u8);
        }
        self.flush()
    }

    // 分配空间方法
    pub fn malloc(&mut self, n: usize) -> usize {
        if let Some(start_index) = self.find_next_n_zeros(n) {
            for i in 0..n {
                let byte_index = (start_index + i) / 8;
                let bit_index = (start_index + i) % 8;
                self.bits[byte_index] = self.toggle_bit(self.bits[byte_index], bit_index as u8);
            }
            let _ = self.flush();
            start_index
        } else {
            let bits_len = self.bits.len();
            let mut count = 0;
            if bits_len > 0 {
                for bit_index in 0..8 {
                    if self.get_bit(self.bits[bits_len - 1], bit_index as u8) == 0 {
                        self.bits[bits_len - 1] = self.toggle_bit(self.bits[bits_len - 1], bit_index as u8);
                        count += 1;
                    } else {
                        count = 0;
                    }
                }
            }

            let start_index = bits_len * 8 - count;
            
            let new_byte_cnt = (n - count + 7) / 8;
            for i in 0..new_byte_cnt {
                if (i + 1) == new_byte_cnt {
                    self.bits.push(255 ^ (255 >> ((n - count) % 8)));
                } else {
                    self.bits.push(255);
                }
            }

            let _ = self.flush();

            start_index
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        self.state.set(0, &self.bits)?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        self.bits = vec![];
        self.state.truncate()?;
        Ok(())
    }

    pub fn print(&self) {
        println!("groups: {}, bytes:{}", self.bits.len(), self.len());
        for (_, &byte) in self.bits.iter().enumerate() {
            println!("{:08b}", byte);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bitmap() {
        let mut bitmap = BitMap::new("/tmp/wtfs_bitemap");
        let _ = bitmap.reset();
        
        assert_eq!(bitmap.len(), 0);
        assert_eq!(bitmap.bits.len(), 0);

        let index = bitmap.malloc(100);
        
        assert_eq!(index, 0);

        let _ = bitmap.free(3, 100);

        let mut bitmap = BitMap::new("/tmp/wtfs_bitemap");
        let index = bitmap.malloc(5);
        assert_eq!(index, 3);

        let index = bitmap.malloc(10);
        assert_eq!(index, 8);
        assert_eq!(bitmap.len(), 104);
    }

    #[test]
    fn test_get_bit() {
        let mut bitmap = BitMap::new("/tmp/wtfs_bitemap");
        let _ = bitmap.reset();
        assert_eq!(bitmap.get_bit(0b00000001, 7), 1);
        assert_eq!(bitmap.get_bit(0b00000010, 6), 1);
        assert_eq!(bitmap.get_bit(0b00000100, 5), 1);
        assert_eq!(bitmap.get_bit(0b10000000, 0), 1);
        assert_eq!(bitmap.get_bit(0b10000000, 1), 0);
        
    }

    #[test]
    fn test_find_next_n_zeros() {
        let mut bitmap = BitMap::new("/tmp/wtfs_bitemap");
        let _ = bitmap.reset();
        bitmap.bits.push(0b11110000);   // 前4位被占用
        assert_eq!(bitmap.find_next_n_zeros(4), Some(4)); // 下一个4个连续的0位从索引4开始
    }

    #[test]
    fn test_malloc_and_free() {
        let mut bitmap = BitMap::new("/tmp/wtfs_bitemap");
        let index = bitmap.malloc(4);
        assert_eq!(index, 0); // 应该从索引0开始分配
        let _ = bitmap.free(index, 4);
        assert_eq!(bitmap.bits[0], 0); // 分配的4位应该被释放
        let _ = bitmap.reset();
    }

    #[test]
    #[should_panic(expected = "bit_index out of range (0-7)")]
    fn test_get_bit_out_of_range() {
        let bitmap = BitMap::new("/tmp/wtfs_bitemap");
        bitmap.get_bit(0b00000001, 8); // 应该触发恐慌，因为索引超出范围
    }
}