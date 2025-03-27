use crate::cmd::Parse;
use crate::error::Error;
use crate::node::Node;
use crate::{Connection, Db, Frame};

use bytes::Bytes;
use std::time::Duration;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Set {
    /// the lookup key
    key: Bytes,

    /// the value to be stored
    value: Bytes,

    /// When to expire the key
    expire: Option<Duration>,
}

impl Set {
    
    pub fn new(key: Bytes, value: Bytes, expire: Option<Duration>) -> Set {
        Set {
            key,
            value,
            expire,
        }
    }

    /// Get the key
    pub fn key(&self) -> &Bytes {
        &self.key
    }

    /// Get the value
    pub fn value(&self) -> &Bytes {
        &self.value
    }

    /// Get the expire
    pub fn expire(&self) -> Option<Duration> {
        self.expire
    }

    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Set> {
        // Read the key to set. This is a required field
        let key = parse.next_bytes()?;

        // Read the value to set. This is a required field.
        let value = parse.next_bytes()?;

        let mut expire = None;

        // Attempt to parse another string.
        match parse.next_string() {
            Ok(s) if s.to_uppercase() == "EX" => {
                // An expiration is specified in seconds. The next value is an
                // integer.
                let secs = parse.next_int()?;
                expire = Some(Duration::from_secs(secs));
            }
            Ok(s) if s.to_uppercase() == "PX" => {
                // An expiration is specified in milliseconds. The next value is
                // an integer.
                let ms = parse.next_int()?;
                expire = Some(Duration::from_millis(ms));
            }
            Ok(_) => return Err(Error::Other("currently `SET` only supports the expiration option".into())),
            Err(Error::EndOfStream) => {}
            Err(err) => return Err(err.into()),
        }

        Ok(Set { key, value, expire })
    }

    #[instrument(skip(self, node, dst))]
    pub(crate) async fn apply(self, node: &Node, dst: &mut Connection) -> crate::Result<()> {
        // Set the value in the shared database state.
        node.set(self.key, self.value, self.expire);

        // Create a success response and write it to `dst`.
        let response = Frame::Simple("OK".to_string());
        debug!(?response);
        dst.write_frame(&response).await.map_err(|err| Error::Response(format!("{:?}", err)))?;

        Ok(())
    }

    /// Converts the command into an equivalent `Frame`.
    ///
    /// This is called by the client when encoding a `Set` command to send to
    /// the server.
    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("set".as_bytes()));
        frame.push_bulk(Bytes::from(self.key));
        frame.push_bulk(self.value);
        if let Some(ms) = self.expire {
            frame.push_bulk(Bytes::from("px".as_bytes()));
            frame.push_int(ms.as_millis() as u64);
        }
        frame
    }
}
