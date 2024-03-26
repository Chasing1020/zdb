use thiserror::Error;

pub type StorageResult<T> = std::result::Result<T, StorageError>;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Encode error: {0}")]
    EncodeError(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
}

impl StorageError {
    pub fn encode_error(msg: String) -> Self {
        StorageError::EncodeError(msg)
    }

    pub fn decode_error(msg: String) -> Self {
        StorageError::DecodeError(msg)
    }
}
