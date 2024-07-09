use std::{convert::TryFrom, ops::Deref, sync::Arc};

use log::{info, error};
use p2p::{service::EventHandler, P2pError};

use crate::{node::Node};


#[derive(Debug, Clone)]
pub struct EventHandlerImpl(Arc<Node>);

impl EventHandlerImpl {
    pub fn new(node: Arc<Node>) -> Self {
        Self(node)
    }
}

impl Deref for EventHandlerImpl {
    type Target = Arc<Node>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl EventHandler for EventHandlerImpl {
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
    // fn handle_inbound_request(&self, request: Vec<u8>) -> Result<Vec<u8>, P2pError> {
    //     let req = Request::try_from(request);
    //     if req.is_err() {
    //         error!("❌ >> [P2P-IN] Invalid request: {:?}", req.err());
    //         return Err(P2pError::RequestRejected);
    //     }
    //     let req = req.unwrap();

    //     info!("📣 >> [P2P-IN] {:?}", req);
    //     let resp = match req.method() {
    //         Method::Height => {
    //             let block_height = self.block_height();
    //             Response::new_block_height_resp(block_height)
    //         }
    //         Method::Blocks => {
    //             let blocks = match req.body.unwrap() {
    //                 request::Body::BlocksReq(req) => self.get_blocks(req.from_number),
    //                 _ => vec![],
    //             };
    //             Response::new_blocks_resp(blocks)
    //         }
    //     };
    //     info!("📣 << [P2P-OUT] {:?}", resp);

    //     Ok(resp.into())
    // }

    // fn handle_broadcast(&self, topic: &str, message: Vec<u8>) {
    //     match Topic::from(topic) {
    //         Topic::Block => {
    //             if let Ok(block) = Block::try_from(message) {
    //                 info!("📣 >> [P2P-IN-BROADCAST] {}", block);
    //                 self.handle_broadcast_block(block);
    //             } else {
    //                 error!("❌ >> [P2P-IN-BROADCAST] Invalid block");
    //             }
    //         }
    //         Topic::Tx => {
    //             if let Ok(tx) = SignedTx::try_from(message) {
    //                 info!("📣 >> [P2P-IN-BROADCAST] {}", tx);
    //                 self.handle_broadcast_tx(tx);
    //             } else {
    //                 error!("❌ >> [P2P-IN-BROADCAST] Invalid tx");
    //             }
    //         }
    //     }
    // }
}

#[derive(Debug)]
enum Topic {
    Block,
    Tx,
}

impl From<&str> for Topic {
    fn from(topic: &str) -> Self {
        if topic == "tx" {
            Self::Tx
        } else {
            Self::Block
        }
    }
}

impl From<Topic> for String {
    fn from(topic: Topic) -> Self {
        match topic {
            Topic::Block => "block".into(),
            Topic::Tx => "tx".into(),
        }
    }
}
