use mineral::kv::hash::HashKv;
use mineral::{KvConfig, StorageConfig};
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

use bytes::Bytes;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(crate) struct DbDropGuard {
    db: Db,
}

#[derive(Debug, Clone)]
pub(crate) struct Db {
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

    pub(crate) fn new() -> DbDropGuard {
        DbDropGuard { db: Db::new() }
    }

    pub(crate) fn db(&self) -> Db {
        self.db.clone()
    }
}

impl Drop for DbDropGuard {
    fn drop(&mut self) {
        self.db.shutdown_purge_task();
    }
}

impl Db {
    pub(crate) fn new() -> Db {
        let conf = KvConfig {
            storage: StorageConfig {
                path: "/tmp/terra/tests/peer/data".to_string(),
                block_size: 1024,
                page_max_cap: 1024 * 1024 * 50,
            },
            wal_path: "/tmp/terra/tests/peer/log".to_string(),
            cache_cap: 1024 * 1024 * 50,
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

    pub(crate) fn get(&self, key: &str) -> Option<Bytes> {
        let mut state = self.shared.state.lock().unwrap();
        state.kv.get(key).map(|data| Bytes::from(data))
    }

    pub(crate) fn set(&self, key: String, value: Bytes, expire: Option<Duration>) {
        let mut state = self.shared.state.lock().unwrap();

        state.kv.setnx(&key, &value.to_vec(), expire);
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