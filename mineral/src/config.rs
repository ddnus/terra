use std::fs;

use serde::Deserialize;

use crate::error::Error;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    /// The path to the data directory.
    pub data_dir: String,
    /// The path to the genesis file.
    pub genesis_file: String,
    /// The address to listen on for HTTP Server.
    pub http_addr: String,
    /// The miner account to receive mining rewards.
    pub author: String,
}