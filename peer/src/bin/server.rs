//! peer server.

use peer::config::Config;
use peer::{server, DEFAULT_PORT};

use clap::Parser;
use tokio::net::TcpListener;
use tokio::signal;

#[tokio::main]
pub async fn main() -> peer::Result<()> {
    let cli = Cli::parse();
    let port = cli.port.unwrap_or(DEFAULT_PORT);
    let config_path = cli.config;

    let config = Config::load(&config_path).unwrap();

    // Bind a TCP listener
    let listener = TcpListener::bind(&format!("{}", config.http_addr)).await?;

    server::run(listener, config, signal::ctrl_c()).await;

    Ok(())
}

#[derive(Parser, Debug)]
#[clap(name = "peer-server", version, author, about = "A peer server")]
struct Cli {
    #[clap(long)]
    port: Option<u16>,
    #[arg(short, long, default_value_t = String::from("config.toml"))]
    config: String,
}