use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::prelude::*;


pub fn compress_data(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

pub fn decompress_data(data: &[u8]) -> Vec<u8> {
    let mut decoder = GzDecoder::new(data);

    let mut decompress_data = Vec::new();

    decoder.read_to_end(&mut decompress_data).unwrap();

    decompress_data

}