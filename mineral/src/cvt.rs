use byteorder::{BigEndian, ByteOrder};

pub fn case_buf_to_u64(buf: &Vec<u8>) -> u64 {
    BigEndian::read_u64(buf)
}

pub fn case_u64_to_buf(index: u64) -> Vec<u8> {
    let mut size_buf = vec![0u8; 8];
    BigEndian::write_u64(&mut size_buf, index);
    size_buf
}

pub fn case_buf_to_u32(buf: &Vec<u8>) -> u32 {
    BigEndian::read_u32(buf)
}

pub fn case_u32_to_buf(index: u32) -> Vec<u8> {
    let mut size_buf = vec![0u8; 8];
    BigEndian::write_u32(&mut size_buf, index);
    size_buf
}