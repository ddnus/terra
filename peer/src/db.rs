use mineral::kv::hash::HashKv;
use mineral::{KvConfig, StorageConfig};
use tokio::sync::Notify;
use tokio::time::Duration;

use bytes::Bytes;
use std::sync::{Arc, Mutex};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct DbDropGuard {
    db: Db,
}

#[derive(Debug, Clone)]
pub struct Db {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {

    state: Mutex<State>,

    background_task: Notify,
}

#[derive(Debug)]
struct State {
    kv: mineral::kv::hash::HashKv,

    shutdown: bool,
}

impl DbDropGuard {

    pub fn new(config: Config) -> DbDropGuard {
        DbDropGuard { db: Db::new(config) }
    }

    pub fn db(&self) -> Db {
        self.db.clone()
    }
}

impl Drop for DbDropGuard {
    fn drop(&mut self) {
        self.db.shutdown_purge_task();
    }
}

impl Db {
    pub fn new(config: Config) -> Db {
        let conf = KvConfig {
            storage: StorageConfig {
                path: config.data_dir.clone() + "data",
                block_size: 1024,
                page_max_cap: 1024 * 1024 * 50,
            },
            wal_path: config.data_dir.clone() + "log",
            cache_cap: 1024 * 1024 * 50,
            cbf_cap: 1024 * 1024 * 50,
            slot_qty: 10000,
        };

        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                kv: HashKv::new(conf),
                shutdown: false,
            }),
            background_task: Notify::new(),
        });

        Db { shared }
    }

    pub fn get(&self, key: &Bytes) -> Option<Bytes> {
        let mut state = self.shared.state.lock().unwrap();
        state.kv.get(&key.to_vec()).map(|data| Bytes::from(data))
    }

    pub fn set(&self, key: Bytes, value: Bytes, expire: Option<Duration>) {
        let mut state = self.shared.state.lock().unwrap();

        state.kv.setnx(&key.to_vec(), &value.to_vec(), expire);
    }

    /// Signals the purge background task to shut down. This is called by the
    /// `DbShutdown`s `Drop` implementation.
    fn shutdown_purge_task(&self) {
        let mut state = self.shared.state.lock().unwrap();
        state.shutdown = true;

        drop(state);
        self.shared.background_task.notify_one();
    }
}

impl Shared {
    fn is_shutdown(&self) -> bool {
        self.state.lock().unwrap().shutdown
    }
}