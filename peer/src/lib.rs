//! A minimal (i.e. very incomplete) implementation of a Redis server and
//! client.
//!
//! The purpose of this project is to provide a larger example of an
//! asynchronous Rust project built with Tokio. Do not attempt to run this in
//! production... seriously.
//!
//! # Layout
//!
//! The library is structured such that it can be used with guides. There are
//! modules that are public that probably would not be public in a "real" redis
//! client library.
//!
//! The major components are:
//!
//! * `server`: Redis server implementation. Includes a single `run` function
//!   that takes a `TcpListener` and starts accepting redis client connections.
//!
//! * `clients/client`: an asynchronous Redis client implementation. Demonstrates how to
//!   build clients with Tokio.
//!
//! * `cmd`: implementations of the supported Redis commands.
//!
//! * `frame`: represents a single Redis protocol frame. A frame is used as an
//!   intermediate representation between a "command" and the byte
//!   representation.

pub mod clients;
pub use clients::{BlockingClient, Client};

pub mod cmd;
pub use cmd::Command;

mod connection;
pub use connection::Connection;

pub mod frame;
pub use frame::Frame;

mod db;
use db::Db;
use db::DbDropGuard;

mod parse;
use parse::Parse;

pub mod server;

pub mod config;

mod shutdown;
use shutdown::Shutdown;

mod error;
use error::Error;

mod p2p;
pub use p2p::P2pClient;

mod state;
mod node;
mod proto;
// mod schema;
// mod wallet;
mod utils;

pub const DEFAULT_PORT: u16 = 6380;

pub type Result<T> = std::result::Result<T, Error>;