use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;

/// The node status, for debugging.
#[derive(Clone, Debug, Default)]
pub struct NodeStatus {
    pub local_peer_id: String,
    pub listened_addresses: Vec<Multiaddr>,
    pub known_peers_count: usize,
    pub known_peers: HashMap<PeerId, Vec<Multiaddr>>,
}
