use libp2p::PeerId;

use crate::{node::NodeStatus, protocol::ResponseType};
use super::Server;
use super::*;

#[derive(Debug)]
pub enum Command {
    SendRequest {
        target: PeerId,
        request: Vec<u8>,
        responder: oneshot::Sender<ResponseType>,
    },
    Broadcast {
        topic: String,
        message: Vec<u8>,
    },
    GetStatus(oneshot::Sender<NodeStatus>),
}

impl <E: EventHandler> Server<E> {
    // Process the next command coming from `Client`.
    pub fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::SendRequest {
                target,
                request,
                responder,
            } => self.handle_outbound_request(target, request, responder),
            Command::Broadcast { topic, message } => self.handle_outbound_broadcast(topic, message),
            Command::GetStatus(responder) => {
                responder.send(self.get_status()).unwrap()
            },
            _ => {
                println!("=============unknown command===========");
            },
        }
    }

    // Store the request_id with the responder so that we can send the response later.
    fn handle_outbound_request(
        &mut self,
        target: PeerId,
        request: Vec<u8>,
        responder: oneshot::Sender<ResponseType>,
    ) {
        let req_id = self
            .network_service
            .behaviour_mut()
            .send_request(&target, request);
        self.pending_outbound_requests.insert(req_id, responder);
    }

    fn get_status(&mut self) -> NodeStatus {
        let known_peers = self.network_service.behaviour_mut().known_peers();
        NodeStatus {
            local_peer_id: self.local_peer_id.to_base58(),
            listened_addresses: self.listened_addresses.clone(),
            known_peers_count: known_peers.len(),
            known_peers,
        }
    }
    

    // Broadcast a message to all peers subscribed to the given topic.
    fn handle_outbound_broadcast(&mut self, topic: String, message: Vec<u8>) {
        let _ = self
            .network_service
            .behaviour_mut()
            .broadcast(topic, message);
    }
    
}

