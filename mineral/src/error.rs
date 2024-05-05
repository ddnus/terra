use std::io::Error as ioError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid wal data format")]
    InvalidWalData,

    #[error("Failed to append wal data")]
    AppendWalDataFailed,

    #[error("Failed to get block data: {0}")]
    BlockDataGetFailed(ioError),

    #[error("Failed to del main block data: {0}")]
    MainDataDelFailed(ioError),

    #[error("Failed to del wal file: {0}")]
    WalDelFailed(ioError),

    #[error("Failed to rename checked wal file: {0}")]
    WalCheckedFailed(ioError),

    #[error("Failed to decode hash slot: {0}")]
    SlotDecodeFailed(String),

    #[error("Failed to encode hash slot: {0}")]
    SlotEncodeFailed(String),
}
