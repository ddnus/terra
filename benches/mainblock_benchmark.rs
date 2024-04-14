use criterion::{criterion_group, criterion_main, Criterion};
use wtfs::storage::mainblock::MainBlock;
use rand::Rng;

fn criterion_benchmark(c: &mut Criterion) {
    let mut mb = MainBlock::new("/tmp/wtfs/benches/", 1155);
    let _ = mb.truncate();

    c.bench_function("test set: 1G-100byte-0", |b| b.iter(|| {
        let secret_number = rand::thread_rng().gen_range(0..1000000);
        let set_buf = vec![1u8; 2138];
        mb.set(secret_number, &set_buf)
    }));

    // init data
    // for i in 0..4000000 {
    //     if i % 100000 == 0 {
    //         println!("current: {}", i);
    //     }
    //     let set_buf = vec![1u8; 2138];
    //     mb.set(i, &set_buf).unwrap();
    // }

    // for i in 0..100000 {
    //     if i % 10000 == 0 {
    //         println!("current: {}", i);
    //     }
    //     let get_buf = mb.get(i).unwrap();
    //     assert_eq!(get_buf.len(), 2138);
    // }

    c.bench_function("test get: 1G-100byte-0", |b| b.iter(|| {
        let secret_number = rand::thread_rng().gen_range(0..1000000);
        mb.get(secret_number)
    }));

    // c.bench_function("test set: 1G-800byte-0", |b| b.iter(|| {
    //     let secret_number = rand::thread_rng().gen_range(0..1024*1024);
    //     let set_buf = vec![1u8; 800];
    //     mb.set(secret_number, &set_buf)
    // }));

    // c.bench_function("test set: 1G-1025byte-1", |b| b.iter(|| {
    //     let secret_number = rand::thread_rng().gen_range(0..1024*1024);
    //     let set_buf = vec![1u8; 1025];
    //     mb.set(secret_number, &set_buf)
    // }));

    // c.bench_function("test set: 1G-1041byte-1", |b| b.iter(|| {
    //     let secret_number = rand::thread_rng().gen_range(0..1024*1024);
    //     let set_buf = vec![1u8; 1041];
    //     mb.set(secret_number, &set_buf)
    // }));

    // c.bench_function("test set: 1G-2031byte-2", |b| b.iter(|| {
    //     let secret_number = rand::thread_rng().gen_range(0..1024*1024);
    //     let set_buf = vec![1u8; 2031];
    //     mb.set(secret_number, &set_buf)
    // }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);