mod cmd;
mod cli;
pub use cli::Client;

mod event;
use std::{collections::HashMap, time::Duration};

pub use event::*;
use cmd::Command;
use libp2p::{
    futures::StreamExt, noise, request_response::OutboundRequestId, tcp, yamux, Multiaddr, PeerId, Swarm
};
use log::info;
use tokio::{select, sync::{mpsc::{self, UnboundedReceiver}, oneshot, OnceCell}, time::{self, Interval}};

use crate::{protocol::{Behaviour, ResponseType}, P2pConfig, P2pError};

/// Create a new p2p node, which consists of a `Client` and a `Server`.
pub fn new<E: EventHandler>(config: P2pConfig) -> Result<(Client, Server<E>), P2pError> {
    let (cmd_sender, cmd_receiver) = mpsc::unbounded_channel();

    let server = Server::new(config, cmd_receiver)?;
    let client = Client { cmd_sender };

    Ok((client, server))
}

pub struct Server<E: EventHandler> {

    local_peer_id: PeerId,
    
    network_service: Swarm<Behaviour>,

    /// The addresses that the server is listening on.
    listened_addresses: Vec<Multiaddr>,
    /// The receiver of commands from the client.
    /// The handler of events from remote peers.
    event_handler: OnceCell<E>,

    cmd_receiver: UnboundedReceiver<Command>,

    /// The ticker to periodically discover new peers.
    discovery_ticker: Interval,
    /// The topics will be hashed when subscribing to the gossipsub protocol,
    /// but we need to keep the original topic names for broadcasting.
    pubsub_topics: Vec<String>,

    pending_outbound_requests: HashMap<OutboundRequestId, oneshot::Sender<ResponseType>>,
}

impl <E: EventHandler> Server<E> {
    pub fn new(
        config: P2pConfig,
        cmd_receiver: UnboundedReceiver<Command>,
    ) -> Result<Self, P2pError> {

        let addr: Multiaddr = config.addr.parse()?;
        let local_key = config.gen_keypair()?;
        let local_peer_id = local_key.public().to_peer_id();
        info!("📣 Local peer id: {:?}", local_peer_id.to_base58());

        let pubsub_topics = config.pubsub_topics;
        // Build the [swarm](https://docs.rs/libp2p/latest/libp2p/struct.Swarm.html)
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        ).unwrap()
        .with_behaviour(|key| {
            let behaviour = Behaviour::new(key.clone(), pubsub_topics.clone(), config.req_resp)
                .unwrap();
            behaviour
        }).unwrap()
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

        // Switch to server mode.
        swarm.add_external_address(addr.clone());
        swarm.listen_on(addr)?;

        // Connect to the boot node if specified.
        if let Some(boot_node) = config.boot_node {
            swarm.dial(boot_node.address())?;
            info!("dial boot nod: {:?}", boot_node);
        }

        // Create a ticker to periodically discover new peers.
        let interval_secs = config.discovery_interval.unwrap_or(30);
        let instant = time::Instant::now() + Duration::from_secs(5);
        let discovery_ticker = time::interval_at(instant, Duration::from_secs(interval_secs));
        let pubsub_topics = pubsub_topics;

        Ok(Self {
            local_peer_id,
            cmd_receiver,
            network_service: swarm,
            event_handler: OnceCell::new(),
            discovery_ticker,
            pubsub_topics,
            listened_addresses: Vec::new(),
            pending_outbound_requests: HashMap::new(),
        })
    }

    /// Run the `Server`.
    pub async fn run(mut self) {
        loop {
            select! {
                // Next discovery process.
                _ = self.discovery_ticker.tick() => {
                    self.network_service.behaviour_mut().discover_peers();
                    // let peers = self.network_service.behaviour_mut().closest_id_peers(self.local_peer_id.to_string(), 10);
                    // info!("=====================peers:{:?}", peers);
                },

                // Next command from the `Client`.
                msg = self.cmd_receiver.recv() => {
                    if let Some(cmd) = msg {
                        self.handle_command(cmd);
                    }
                },
                // Next event from `Swarm`.
                event = self.network_service.select_next_some() => {
                    self.handle_swarm_event(event);
                },
            }
        }
    }
    
}
