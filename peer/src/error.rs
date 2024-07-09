use p2p::P2pError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Other error: {0}")]
    Other(String),
    
    #[error("Config file not exist: {0}")]
    ConfigNotExist(String),
    
    #[error(transparent)]
    InvalidConfig(#[from] toml::de::Error),

    #[error("Unknown {0} sub command: {1}")]
    UnknownCommand(String, String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),
    
    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid type of frame:{0}")]
    InvalidFrameType(String),

    #[error("End of the frame steam")]
    EndOfStream,

    #[error("Response error: {0}")]
    Response(String),

    #[error("P2P error: {0}")]
    P2pError(P2pError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parser incomplete")]
    Incomplete,
}

impl From<P2pError> for Error {
    fn from(err: P2pError) -> Error {
        Error::P2pError(err)
    }
}
