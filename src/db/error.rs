use std::io;

use thiserror::Error;

pub type StorageResult<T> = std::result::Result<T, StorageError>;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("EOF")]
    Eof,
    #[error("Not found: {0:?}")]
    NotFound(String),
    #[error("Corruption: {0:?}")]
    Corruption(String),
    #[error("Not supported: {0:?}")]
    NotSupported(String),
    #[error("Invalid argument: {0:?}")]
    InvalidArgument(String),
    #[error(transparent)]
    IOError(#[from] io::Error),
}
