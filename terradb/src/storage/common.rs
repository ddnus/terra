use std::{fs::{self}, path::Path};
use byteorder::{BigEndian, ByteOrder};

pub fn case_buf_to_u64(buf: &Vec<u8>) -> u64 {
    BigEndian::read_u64(buf)
}

pub fn case_u64_to_buf(index: u64) -> Vec<u8> {
    let mut size_buf = vec![0u8; 8];
    BigEndian::write_u64(&mut size_buf, index);
    size_buf
}

pub fn build_path(path: &str, file_name: &str) -> String {
    let binding = Path::new(path);
    if !binding.exists() {
        let _ = fs::create_dir_all(path);
    }
    let binding = binding.join(file_name);
    let datablock_path = binding.to_str().unwrap();
    datablock_path.to_string()
}