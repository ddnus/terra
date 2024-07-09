
use std::ops::Deref;

use log::info;
use p2p::{Client};

use crate::Error;

/// `P2pClient` is a wrapper around `tinyp2p::Client` that implements the `Peer` trait.
#[derive(Debug, Clone)]
pub struct P2pClient(Client);

impl P2pClient {
    pub fn new(client: Client) -> Self {
        Self(client)
    }
}

// Implement `Deref` so that we can call `Client` methods on `P2pClient`.
impl Deref for P2pClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl P2pClient {
    async fn known_peers(&self) -> Vec<String> {
        let peers = self.get_known_peers().await;
        info!("📣 Known peers {:?}", peers);
        peers
    }
}
