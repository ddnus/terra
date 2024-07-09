use log::info;
use p2p::{service::{self, EventHandler}, Client, PeerIdWithMultiaddr};

use clap::Parser;
use p2p::P2pConfig;

use std::{thread, time::Duration};

use tokio::task;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let mut config = P2pConfig {
        addr: "/ip4/0.0.0.0/tcp/0".to_string(),
        pubsub_topics: vec!["block".to_string(), "tx".to_string()],
        boot_node: Some("/ip4/127.0.0.1/tcp/51529/p2p/12D3KooWKkW4HnGFwNXAJeLyyxEbEFMEYB5gPKkNDRzycuX4cKCm".parse().unwrap()),
        ..Default::default()
    };
    config.pubsub_topics = vec!["block".to_string(), "tx".to_string()];
    if let Some(addr) = std::env::args().nth(1) {
        config.boot_node = addr.parse().ok();
    }

    let (client, mut server) = p2p::new(config).unwrap();
    server.set_event_handler(Handler);

    // Run the p2p server
    task::spawn(server.run());

    // Periodically print the node status.
    let client_clone = client.clone();
    thread::spawn(move || get_node_status(client_clone));

    // Periodically send a request to one of the known peers.
    let client_clone = client.clone();
    thread::spawn(move || request(client_clone));

    // Periodically make a broadcast to the network.
    broadcast(client);
}

#[derive(Debug)]
struct Handler;

impl EventHandler for Handler {
    fn handle_inbound_request(&self, request: Vec<u8>) -> Result<Vec<u8>, p2p::P2pError> {
        info!(
            "📣 <<<< Inbound request: {:?}",
            String::from_utf8_lossy(request.as_slice())
        );
        Ok(request)
    }

    fn handle_broadcast(&self, topic: &str, message: Vec<u8>) {
        info!(
            "📣 <<<< Inbound broadcast: {:?} {:?}",
            topic,
            String::from_utf8_lossy(message.as_slice())
        );
    }
}

async fn get_node_status(client: Client) {
    let dur = Duration::from_secs(7);
    loop {
        thread::sleep(dur);
        let node_status = client.get_node_status().await;
        info!("📣 Node status: {:?}", node_status);
    }
}

async fn request(client: Client) {
    let dur = Duration::from_secs(11);
    loop {
        thread::sleep(dur);
        let known_peers = client.get_known_peers().await;
        if known_peers.len() > 0 {
            let target = &known_peers[0];
            let request = "Hello, request!";
            info!("📣 >>>> Outbound request: {:?}", request);
            let response = client
                .blocking_request(target, request.as_bytes().to_vec())
                .unwrap();
            info!(
                "📣 <<<< Inbound response: {:?}",
                String::from_utf8_lossy(&response)
            );
        }
    }
}

fn broadcast(client: Client) {
    let dur = Duration::from_secs(13);
    loop {
        thread::sleep(dur);
        let topic = "block";
        let message = "Hello, a new block!";
        info!("📣 >>>> Outbound broadcast: {:?} {:?}", topic, message);
        let _ = client.broadcast(topic, message.as_bytes().to_vec());
    }
}