use itertools::Itertools;
use libp2p::gossipsub::TopicHash;
use libp2p::request_response::OutboundFailure;
use libp2p::Multiaddr;
use log::{error, info, warn};
use tokio::sync::oneshot;
use crate::{protocol::*, P2pError};

use libp2p::{
    request_response::{self, OutboundRequestId, ResponseChannel},
    swarm::SwarmEvent,
    PeerId,
    identify,
    ping,
    gossipsub,
};

use std::io;
use std::fmt::Debug;
use super::Server;

#[derive(Debug)]
pub(crate) enum Event {
    InboundRequest {
        request: Vec<u8>,
        channel: ResponseChannel<Result<Vec<u8>, ()>>,
    },
}

pub trait EventHandler: Debug + Send + 'static {
    /// Handles an inbound request from a remote peer.
    fn handle_inbound_request(&self, request: Vec<u8>) -> Result<Vec<u8>, P2pError>;

    /// Handles an broadcast message from a remote peer.
    fn handle_broadcast(&self, topic: &str, message: Vec<u8>);
}

impl <E: EventHandler> Server<E> {

    /// Set the handler of events from remote peers.
    pub fn set_event_handler(&mut self, handler: E) {
        self.event_handler.set(handler).unwrap();
    }
    
    // Process the next event coming from `Swarm`.
    pub fn handle_swarm_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        let behaviour_ev = match event {
            SwarmEvent::Behaviour(ev) => ev,

            SwarmEvent::NewListenAddr { address, .. } => {
                info!("📣 P2P node listening on {:?}", address);
                return self.update_listened_addresses();
            }

            SwarmEvent::ListenerClosed {
                reason, addresses, ..
            } => return Self::log_listener_close(reason, addresses),

            // Can't connect to the `peer`, remove it from the DHT.
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(peer),
                ..
            } => return self.network_service.behaviour_mut().remove_peer(&peer),

            _ => return,
        };

        self.handle_behaviour_event(behaviour_ev);
    }

    fn handle_behaviour_event(&mut self, ev: BehaviourEvent) {
        match ev {
            // See https://docs.rs/libp2p/latest/libp2p/kad/index.html#important-discrepancies
            BehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info: identify::Info { listen_addrs, .. },
            }) => self.add_addresses(&peer_id, listen_addrs),

            // The remote peer is unreachable, remove it from the DHT.
            BehaviourEvent::Ping(ping::Event {
                peer,
                result: Err(_),
                ..
            }) => self.network_service.behaviour_mut().remove_peer(&peer),

            BehaviourEvent::ReqResp(request_response::Event::Message {
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                ..
            }) => self.handle_inbound_request(request, channel),

            BehaviourEvent::ReqResp(request_response::Event::Message {
                message:
                    request_response::Message::Response {
                        request_id,
                        response,
                    },
                ..
            }) => self.handle_inbound_response(request_id, response),

            BehaviourEvent::ReqResp(request_response::Event::OutboundFailure {
                request_id,
                error,
                ..
            }) => self.handle_outbound_failure(request_id, error),

            BehaviourEvent::Pubsub(gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            }) => self.handle_inbound_broadcast(message),

            _ => {}
        }
    }

    // Inbound requests are handled by the `EventHandler` which is provided by the application layer.
    fn handle_inbound_request(&mut self, request: Vec<u8>, ch: ResponseChannel<ResponseType>) {
        if let Some(handler) = self.event_handler.get() {
            let response = handler.handle_inbound_request(request).map_err(|_| ());
            self.network_service
                .behaviour_mut()
                .send_response(ch, response);
        }
    }

    // Inbound broadcasts are handled by the `EventHandler` which is provided by the application layer.
    fn handle_inbound_broadcast(&mut self, message: gossipsub::Message) {
        if let Some(handler) = self.event_handler.get() {
            let topic_hash = message.topic;
            match self.get_topic(&topic_hash) {
                Some(topic) => handler.handle_broadcast(&topic, message.data),
                None => {
                    warn!("❗ Received broadcast for unknown topic: {:?}", topic_hash);
                    debug_assert!(false);
                }
            }
        }
    }

     // An outbound request failed, notify the application layer.
     fn handle_outbound_failure(&mut self, request_id: OutboundRequestId, error: OutboundFailure) {
        if let Some(responder) = self.pending_outbound_requests.remove(&request_id) {
            error!("❌ Outbound request failed: {:?}", error);
            let _ = responder.send(Err(()));
        } else {
            warn!("❗ Received failure for unknown request: {}", request_id);
            debug_assert!(false);
        }
    }

    // An inbound response was received, notify the application layer.
    fn handle_inbound_response(&mut self, request_id: OutboundRequestId, response: ResponseType) {
        if let Some(responder) = self.pending_outbound_requests.remove(&request_id) {
            let _ = responder.send(response);
        } else {
            warn!("❗ Received response for unknown request: {}", request_id);
            debug_assert!(false);
        }
    }

    fn add_addresses(&mut self, peer_id: &PeerId, addresses: Vec<Multiaddr>) {
        for addr in addresses.into_iter().unique() {
            self.network_service
                .behaviour_mut()
                .add_address(peer_id, addr);
        }
    }

    fn update_listened_addresses(&mut self) {
        self.listened_addresses = self
            .network_service
            .listeners()
            .map(ToOwned::to_owned)
            .collect();
    }

    /// Returns the topic name for the given topic hash.
    fn get_topic(&self, topic_hash: &TopicHash) -> Option<String> {
        for t in &self.pubsub_topics {
            let topic = gossipsub::IdentTopic::new(t);
            if topic.hash() == *topic_hash {
                return Some(t.clone());
            }
        }

        None
    }

    fn log_listener_close(reason: io::Result<()>, addresses: Vec<Multiaddr>) {
        let addrs = addresses
            .into_iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        match reason {
            Ok(()) => {
                info!("📣 Listener ({}) closed gracefully", addrs)
            }
            Err(e) => {
                error!("❌ Listener ({}) closed: {}", addrs, e)
            }
        }
    }
}
