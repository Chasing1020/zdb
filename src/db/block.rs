pub mod builder;
pub mod iterator;

use bytes::{Buf, BufMut, Bytes};

use super::error::{StorageError, StorageResult};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionType {
    #[default]
    None = 0,
    Snappy = 1,
    Lz4 = 2,
    Zstd = 3,
}

impl CompressionType {
    pub fn encode(&self, buf: &mut impl BufMut) {
        buf.put_u8(*self as u8);
    }

    pub fn decode(buf: &mut impl Buf) -> StorageResult<Self> {
        let byte = buf.get_u8();
        match byte {
            0 => Ok(Self::None),
            1 => Ok(Self::Snappy),
            2 => Ok(Self::Lz4),
            3 => Ok(Self::Zstd),
            _ => Err(StorageError::decode_error(format!(
                "Invalid compression type: {}",
                byte
            ))),
        }
    }
}

// Block is the basic unit of storage in the SSTable file.
// It contains the uncompressed data and restart points.
pub struct Block {
    // uncompressed data
    pub uncompressed_data: Bytes,
    // restart points
    pub restart_points: Vec<u32>,
    // restart interval
    pub restart_interval: usize,
    // compression type
    pub compression_type: CompressionType,
}
