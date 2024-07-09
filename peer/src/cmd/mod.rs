mod get;
pub use get::Get;

mod set;
pub use set::Set;

mod ping;
pub use ping::Ping;

mod unknown;
pub use unknown::Unknown;

mod peer;
pub use peer::Peer;

use crate::{cmd, node::Node, Connection, Db, Frame, Parse, Shutdown, error::Error};

/// Enumeration of supported Redis commands.
///
/// Methods called on `Command` are delegated to the command implementation.
#[derive(Debug)]
pub enum Command {
    Get(Get),
    Set(Set),
    Ping(Ping),
    Unknown(Unknown),
    Peer(Peer),
}

impl Command {
    /// On success, the command value is returned, otherwise, `Err` is returned.
    pub fn from_frame(frame: Frame) -> crate::Result<Command> {
        // result in an error being returned.
        let mut parse = Parse::new(frame)?;

        let command_name = parse.next_string()?.to_lowercase();

        let command = match &command_name[..] {
            "get" => Command::Get(Get::parse_frames(&mut parse)?),
            "set" => Command::Set(Set::parse_frames(&mut parse)?),
            "ping" => Command::Ping(Ping::parse_frames(&mut parse)?),
            "peer" => Command::Peer(Peer::parse_frames(&mut parse)?),
            _ => {
                return Ok(Command::Unknown(Unknown::new(command_name)));
            }
        };

        parse.finish()?;

        Ok(command)
    }

    pub(crate) async fn apply(
        self,
        node: &Node,
        dst: &mut Connection,
        shutdown: &mut Shutdown,
    ) -> crate::Result<()> {
        use Command::*;

        match self {
            Get(cmd) => cmd.apply(node, dst).  await,
            Set(cmd) => cmd.apply(node, dst).await,
            Ping(cmd) => cmd.apply(dst).await,
            Peer(cmd) => cmd.apply(node, dst).await,
            Unknown(cmd) => cmd.apply(dst).await,
        }
    }

    /// Returns the command name
    pub(crate) fn get_name(&self) -> &str {
        match self {
            Command::Get(_) => "get",
            Command::Set(_) => "set",
            Command::Ping(_) => "ping",
            Command::Peer(_) => "peer",
            Command::Unknown(cmd) => cmd.get_name(),
        }
    }
}
