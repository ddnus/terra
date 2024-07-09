mod client;
mod event;

pub use client::P2pClient;
pub use event::EventHandlerImpl;
use p2p::{config::P2pConfig, service::Server};

use crate::error::Error;

/// Creates a new p2p client, event loop, and server.
pub fn new(config: P2pConfig) -> Result<(P2pClient, Server<EventHandlerImpl>), Error> {
    let (client, p2p_server) = p2p::new(config)?;
    let p2p_client = P2pClient::new(client);

    Ok((p2p_client, p2p_server))
}
