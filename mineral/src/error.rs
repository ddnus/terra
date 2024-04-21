#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid wal data format")]
    InvalidWalData,

    #[error("Failed to append wal data")]
    AppendWalDataFailed,

}
