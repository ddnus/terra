
#[derive(Clone, Debug, Default)]
pub struct StorageConfig {
    // 数据文件存储路径
    pub path: String,
    // 数据块读取容量
    pub block_size: usize,
    // 页最大容量
    pub page_max_cap: usize,
}

#[derive(Clone, Debug, Default)]
pub struct KvConfig {
    pub storage: StorageConfig,
    pub wal_path: String,
    // hash lru存储容量
    pub cache_cap: usize,
    // 变更缓冲容量大小设置
    pub cbf_cap: usize,
    // hash槽位数量
    pub slot_qty: u32,
}