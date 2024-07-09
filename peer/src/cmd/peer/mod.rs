mod basic;
// pub use basic::*;
pub use basic::PeerBasic;
use bytes::Bytes;

use crate::{error::Error, node::Node, parse::Parse, Connection, Frame};

use super::Unknown;

use tracing::instrument;

#[derive(Debug)]
pub enum Peer {
    Basic(PeerBasic),
    Unknown(Unknown),
}

impl Peer {
    pub fn new(tag: &str) -> Peer {
        if tag == "basic" {
            Peer::Basic(PeerBasic)
        } else {
            Peer::Unknown(Unknown::new(tag.to_string()))
        }
    }

    pub fn parse_frames(parse: &mut Parse) -> crate::Result<Peer> {
        let command_name = parse.next_string()?.to_lowercase();
        
        let command = match &command_name[..] {
            "basic" => Peer::Basic(PeerBasic),
            _ => {
                Peer::Unknown(Unknown::new(command_name))
            },
        };

        Ok(command)
    }

    #[instrument(skip(self, node, dst))]
    pub async fn apply(self, node: &Node, dst: &mut Connection) -> crate::Result<()> {
        return match self {
            Peer::Basic(peer_basic) => {
                peer_basic.apply(node, dst).await
            },
            Peer::Unknown(unknown) => {
                Err(Error::UnknownCommand("peer".to_string(), format!("{:?}", unknown)))
            },
        }
    }

    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("peer".as_bytes()));
        let sub_frame = self.get_sub_frame();
        frame.merge_frame(sub_frame);
        frame
    }

    fn get_sub_frame(self) -> Frame {
        match self {
            Peer::Basic(peer_basic) => {
                peer_basic.into_frame()
            },
            _ => {
                Frame::Null
            },
        }
    }
}