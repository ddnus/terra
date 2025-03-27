use crate::{error::Error, node::Node, Connection, Db, Frame, Parse};

use bytes::Bytes;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Get {
    key: Bytes,
}

impl Get {
    /// Create a new `Get` command which fetches `key`.
    pub fn new(key: Bytes) -> Get {
        Get {
            key: key,
        }
    }

    /// Get the key
    pub fn key(&self) -> &Bytes {
        &self.key
    }

    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Get> {
        let key = parse.next_bytes()?;

        Ok(Get { key })
    }

    #[instrument(skip(self, node, dst))]
    pub(crate) async fn apply(self, node: &Node, dst: &mut Connection) -> crate::Result<()> {
        // Get the value from the shared database state
        let response = if let Some(value) = node.get(&self.key) {
            Frame::Bulk(value)
        } else {
            // If there is no value, `Null` is written.
            Frame::Null
        };

        debug!(?response);

        // Write the response back to the client
        dst.write_frame(&response).await?;

        Ok(())
    }

    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("get".as_bytes()));
        frame.push_bulk(Bytes::from(self.key));
        frame
    }
}
