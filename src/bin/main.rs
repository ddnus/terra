use wtfs::bytemap;
fn main() {
    let mut bitmap = bytemap::BitMap::new("/tmp/wtfs_bitemap");

    let start_index = bitmap.malloc(10);
    
    println!("start index: {}", start_index);

}