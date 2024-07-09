use crate::{node::Node, Connection, Db, Frame, Parse};

use bytes::Bytes;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct PeerBasic;

impl PeerBasic {
    pub fn new() -> PeerBasic {
        PeerBasic
    }

    pub fn parse_frames(parse: &mut Parse) -> crate::Result<PeerBasic> {
        Ok(PeerBasic)
    }

    #[instrument(skip(self, node, dst))]
    pub async fn apply(self, node: &Node, dst: &mut Connection) -> crate::Result<()> {
        let response = if let Some(value) = node.peer_basic().await {
            let mut frame = Frame::array();
            let _ = value.iter().for_each(|s| frame.push_string(s.clone()));
            frame
        } else {
            Frame::Null
        };

        debug!(?response);

        dst.write_frame(&response).await?;

        Ok(())
    }

    pub fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("basic".as_bytes()));
        frame
    }
}
