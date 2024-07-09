fn main() {
    prost_build::Config::new()
        .out_dir("src/proto")
        .compile_protos(&["v1.proto"], &["src/proto"])
        .unwrap();

    prost_build::Config::new()
        .out_dir("src/proto")
        .compile_protos(&["account.v1.proto"], &["src/proto"])
        .unwrap();
}
