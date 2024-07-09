use libp2p::kad::{KBucketDistance, KBucketKey, QueryId, RecordKey};

use libp2p::{
    request_response::{self, ResponseChannel, OutboundRequestId},
    kad,
    ping,
    identify,
    identity::Keypair,
    Multiaddr,
    multiaddr::Protocol,
    swarm::NetworkBehaviour,
    PeerId,
    gossipsub::{self, IdentTopic},
};

use log::debug;
use crate::{config::ReqRespConfig, error::P2pError};

use std::collections::{hash_map::DefaultHasher, HashMap};
use std::vec;
use std::{
    io,
    hash::{Hash, Hasher},
    net::IpAddr,
    time::Duration,
};
mod req_resp;
use either::Either;
use req_resp::GenericCodec;
pub use req_resp::ResponseType;
use void::Void;

pub type BehaviourErr = Either<Either<Either<Either<io::Error, io::Error>, Void>, Void>, Void>;

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    kad: kad::Behaviour<kad::store::MemoryStore>,

    identify: identify::Behaviour,
    ping: ping::Behaviour,

    // `req_resp` is used for sending requests and responses.
    req_resp: request_response::Behaviour<GenericCodec>,

    pubsub: gossipsub::Behaviour,
}

impl Behaviour {
    pub fn new(
        local_key: Keypair,
        pubsub_topics: Vec<String>,
        req_resp_config: Option<ReqRespConfig>,
    ) -> Result<Self, P2pError> {

        let local_pubkey = local_key.public();
        let local_id = local_pubkey.to_peer_id();

        let kad_behaviour = kad::Behaviour::new(
            local_id,
            kad::store::MemoryStore::new(local_id),
        );

        let id_behaviour = identify::Behaviour::new(identify::Config::new(
            "/terra/identify/1.0.0".to_string(),
            local_pubkey,
        ));

        Ok(Self {
            kad: kad_behaviour,
            identify: id_behaviour,
            ping: ping::Behaviour::default(),
            req_resp: Self::new_req_resp(req_resp_config),
            pubsub: Self::new_gossipsub(local_key, pubsub_topics)?,
        })
    }

    pub fn discover_peers(&mut self) {
        if self.known_peers().is_empty() {
            debug!("☕ Discovery process paused due to no boot node");
        } else {
            debug!("☕ Starting a discovery process");
            let _ = self.kad.bootstrap();
        }
    }

    pub fn known_peers(&mut self) -> HashMap<PeerId, Vec<Multiaddr>> {
        let mut peers = HashMap::new();
        for b in self.kad.kbuckets() {
            for e in b.iter() {
                peers.insert(*e.node.key.preimage(), e.node.value.clone().into_vec());
            }
        }

        peers
    }

    pub fn closest_id_peers(&mut self, key: String, limit: u8) -> Vec<(PeerId, Vec<Multiaddr>, KBucketDistance)> {
        let key = KBucketKey::new(key.as_bytes());
        let mut peers: Vec<(PeerId, Vec<Multiaddr>, KBucketDistance)> = vec![];
        let mut get_num = 0;
        for b in self.kad.kbuckets() {
            for e in b.iter() {
                if get_num > limit {
                    continue;
                }

                let peer_id = *e.node.key.preimage();
                let key_tmp = KBucketKey::new(peer_id.to_bytes());
                
                peers.push((*e.node.key.preimage(), e.node.value.clone().into_vec(), key_tmp.distance(&key)));
                get_num += 1;
            }
        }
        peers
    }

    // pub fn get_closest_peers(&self, key: &str) {
    //     let record_key = RecordKey::new(&key);
    //     let target = KBucketKey::new(record_key.clone());
    //     let peers = self.kad.get_closest_peers(&target);
    // }

    // pub fn get_closest_lock_peers(&self, key: &str) {
    //     let record_key = RecordKey::new(&key);
    //     let target = KBucketKey::new(record_key.clone());
    //     self.kad.get_closest_local_peers(&target).collect()
    // }

    pub fn send_request(&mut self, target: &PeerId, request: Vec<u8>) -> OutboundRequestId {
        self.req_resp.send_request(target, request)
    }

    pub fn send_response(&mut self, ch: ResponseChannel<ResponseType>, response: ResponseType) {
        let _ = self.req_resp.send_response(ch, response);
    }

    pub fn broadcast(&mut self, topic: String, message: Vec<u8>) -> Result<(), P2pError> {
        let topic = gossipsub::IdentTopic::new(topic);
        self.pubsub.publish(topic, message)?;

        Ok(())
    }

    pub fn query_mut(&mut self, id: &QueryId) -> Option<kad::QueryMut> {
        self.kad.query_mut(id)
    }

    pub fn add_address(&mut self, peer_id: &PeerId, addr: Multiaddr) {
        if can_add_to_dht(&addr) {
            debug!("☕ Adding address {} from {:?} to the DHT.", addr, peer_id);
            self.kad.add_address(peer_id, addr);
        }
    }

    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        debug!("☕ Removing peer {} from the DHT.", peer_id);
        self.kad.remove_peer(peer_id);
    }

    fn new_req_resp(config: Option<ReqRespConfig>) -> request_response::Behaviour<GenericCodec> {
        if let Some(config) = config {
            return req_resp::BehaviourBuilder::new()
                .with_connection_keep_alive(config.connection_keep_alive)
                .with_request_timeout(config.request_timeout)
                .with_max_request_size(config.max_request_size)
                .with_max_response_size(config.max_response_size)
                .build();
        }

        req_resp::BehaviourBuilder::default().build()
    }

    fn new_gossipsub(
        local_key: Keypair,
        topics: Vec<String>,
    ) -> Result<gossipsub::Behaviour, P2pError> {
        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .map_err(|err| P2pError::PubsubBuildError(err.to_string()))?;

        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key),
            gossipsub_config,
        )
        .map_err(|err| P2pError::PubsubBuildError(err.to_string()))?;

        for t in topics {
            let topic = IdentTopic::new(t);
            gossipsub.subscribe(&topic)?;
        }

        Ok(gossipsub)
    }

    pub fn set_mode(&mut self, mode: Option<kad::Mode>) {
        self.kad.set_mode(mode)
    }

    pub fn get_providers(&mut self, key: RecordKey) -> QueryId{
        self.kad.get_providers(key)
    }

    pub fn start_providing(&mut self, key: RecordKey) -> Result<QueryId, kad::store::Error> {
        self.kad.start_providing(key)
    }
}

fn can_add_to_dht(addr: &Multiaddr) -> bool {
    let ip = match addr.iter().next() {
        Some(Protocol::Ip4(ip)) => IpAddr::V4(ip),
        Some(Protocol::Ip6(ip)) => IpAddr::V6(ip),
        Some(Protocol::Dns(_)) | Some(Protocol::Dns4(_)) | Some(Protocol::Dns6(_)) => return true,
        _ => return false,
    };

    !ip.is_loopback() && !ip.is_unspecified()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_add_to_dht() {
        let ip4_loopback: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();
        let ip6_loopback: Multiaddr = "/ip6/::1/tcp/8000".parse().unwrap();
        let ip4_unspecified: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse().unwrap();
        let ip6_unspecified: Multiaddr = "/ip6/::/tcp/0".parse().unwrap();
        let ip4: Multiaddr = "/ip4/192.168.0.10/tcp/8000".parse().unwrap();
        let ip6: Multiaddr = "/ip6/fe80::1/tcp/8000".parse().unwrap();
        let domain_name: Multiaddr = "/dns4/example.com/tcp/8000".parse().unwrap();

        assert!(!can_add_to_dht(&ip4_loopback));
        assert!(!can_add_to_dht(&ip6_loopback));
        assert!(!can_add_to_dht(&ip4_unspecified));
        assert!(!can_add_to_dht(&ip6_unspecified));
        assert!(can_add_to_dht(&ip4));
        assert!(can_add_to_dht(&ip6));
        assert!(can_add_to_dht(&domain_name));
    }
}

