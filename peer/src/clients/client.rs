//! Minimal Redis client implementation
//!
//! Provides an async connect and methods for issuing the supported commands.

use crate::cmd::{Get, Ping, Set, Peer};
use crate::error::Error;
use crate::{frame, Connection, Frame};

use async_stream::try_stream;
use bytes::Bytes;
use std::io::ErrorKind;
use std::time::Duration;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_stream::Stream;
use tracing::{debug, instrument};

/// Established connection with a Redis server.
///
/// Backed by a single `TcpStream`, `Client` provides basic network client
/// functionality (no pooling, retrying, ...). Connections are established using
/// the [`connect`](fn@connect) function.
///
/// Requests are issued using the various methods of `Client`.
pub struct Client {
    connection: Connection,
}

pub struct Subscriber {
    /// The subscribed client.
    client: Client,

    /// The set of channels to which the `Subscriber` is currently subscribed.
    subscribed_channels: Vec<String>,
}

/// A message received on a subscribed channel.
#[derive(Debug, Clone)]
pub struct Message {
    pub channel: String,
    pub content: Bytes,
}

impl Client {
    /// Establish a connection with the Redis server located at `addr`.
    ///
    /// `addr` may be any type that can be asynchronously converted to a
    /// `SocketAddr`. This includes `SocketAddr` and strings. The `ToSocketAddrs`
    /// trait is the Tokio version and not the `std` version.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use peer::clients::Client;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = match Client::connect("localhost:6379").await {
    ///         Ok(client) => client,
    ///         Err(_) => panic!("failed to establish connection"),
    ///     };
    /// # drop(client);
    /// }
    /// ```
    ///
    pub async fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<Client> {

        let socket = TcpStream::connect(addr).await?;

        let connection = Connection::new(socket);

        Ok(Client { connection })
    }

    #[instrument(skip(self))]
    pub async fn ping(&mut self, msg: Option<Bytes>) -> crate::Result<Bytes> {
        let frame = Ping::new(msg).into_frame();
        debug!(request = ?frame);
        self.connection.write_frame(&frame).await?;

        match self.read_response().await? {
            Frame::Simple(value) => Ok(value.into()),
            Frame::Bulk(value) => Ok(value),
            frame => Err(frame.to_error()),
        }
    }

    #[instrument(skip(self))]
    pub async fn get(&mut self, key: Bytes) -> crate::Result<Option<Bytes>> {
        // Create a `Get` command for the `key` and convert it to a frame.
        let frame = Get::new(key).into_frame();

        debug!(request = ?frame);

        self.connection.write_frame(&frame).await?;

        match self.read_response().await? {
            Frame::Simple(value) => Ok(Some(value.into())),
            Frame::Bulk(value) => Ok(Some(value)),
            Frame::Null => Ok(None),
            frame => Err(frame.to_error()),
        }
    }

    #[instrument(skip(self))]
    pub async fn set(&mut self, key: Bytes, value: Bytes) -> crate::Result<()> {
        self.set_cmd(Set::new(key, value, None)).await
    }

    #[instrument(skip(self))]
    pub async fn set_expires(
        &mut self,
        key: Bytes,
        value: Bytes,
        expiration: Duration,
    ) -> crate::Result<()> {
        self.set_cmd(Set::new(key, value, Some(expiration))).await
    }

    pub async fn peer_basic(&mut self) -> crate::Result<Option<Bytes>> {

        let frame = Peer::new("basic").into_frame();

        debug!(request = ?frame);

        self.connection.write_frame(&frame).await?;

        match self.read_response().await? {
            Frame::Simple(value) => Ok(Some(value.into())),
            Frame::Bulk(value) => Ok(Some(value)),
            Frame::Null => Ok(None),
            Frame::Array(frames) => {
                let strings: Vec<String> = frames.iter()
                    .map(|frame| frame.to_string())
                    .collect();
                Ok(Some(strings.join("\n").into()))
            },
            frame => Err(frame.to_error()),
        }
    }

    /// The core `SET` logic, used by both `set` and `set_expires.
    async fn set_cmd(&mut self, cmd: Set) -> crate::Result<()> {
        // Convert the `Set` command into a frame
        let frame = cmd.into_frame();

        debug!(request = ?frame);

        // Write the frame to the socket. This writes the full frame to the
        // socket, waiting if necessary.
        self.connection.write_frame(&frame).await?;

        // Wait for the response from the server. On success, the server
        // responds simply with `OK`. Any other response indicates an error.
        match self.read_response().await? {
            Frame::Simple(response) if response == "OK" => Ok(()),
            frame => Err(frame.to_error()),
        }
    }

    /// Reads a response frame from the socket.
    ///
    /// If an `Error` frame is received, it is converted to `Err`.
    async fn read_response(&mut self) -> crate::Result<Frame> {
        let response = self.connection.read_frame().await?;

        debug!(?response);

        match response {
            // Error frames are converted to `Err`
            Some(Frame::Error(msg)) => Err(msg.into()),
            Some(frame) => Ok(frame),
            None => Err(Error::InvalidFrameType("connection reset by server".to_string()))
        }
    }
}

impl Subscriber {
    /// Returns the set of channels currently subscribed to.
    pub fn get_subscribed(&self) -> &[String] {
        &self.subscribed_channels
    }

    /// Receive the next message published on a subscribed channel, waiting if
    /// necessary.
    ///
    /// `None` indicates the subscription has been terminated.
    pub async fn next_message(&mut self) -> crate::Result<Option<Message>> {
        match self.client.connection.read_frame().await? {
            Some(mframe) => {
                debug!(?mframe);

                match mframe {
                    Frame::Array(ref frame) => match frame.as_slice() {
                        [message, channel, content] if *message == "message" => Ok(Some(Message {
                            channel: channel.to_string(),
                            content: Bytes::from(content.to_string()),
                        })),
                        _ => Err(mframe.to_error()),
                    },
                    frame => Err(frame.to_error()),
                }
            }
            None => Ok(None),
        }
    }

    /// Convert the subscriber into a `Stream` yielding new messages published
    /// on subscribed channels.
    ///
    /// `Subscriber` does not implement stream itself as doing so with safe code
    /// is non trivial. The usage of async/await would require a manual Stream
    /// implementation to use `unsafe` code. Instead, a conversion function is
    /// provided and the returned stream is implemented with the help of the
    /// `async-stream` crate.
    pub fn into_stream(mut self) -> impl Stream<Item = crate::Result<Message>> {
        // Uses the `try_stream` macro from the `async-stream` crate. Generators
        // are not stable in Rust. The crate uses a macro to simulate generators
        // on top of async/await. There are limitations, so read the
        // documentation there.
        try_stream! {
            while let Some(message) = self.next_message().await? {
                yield message;
            }
        }
    }

}
