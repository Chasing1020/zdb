pub mod builder;
pub mod iterator;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use integer_encoding::VarInt;

use super::{
    error::{StorageError, StorageResult},
    format::key::{TableKey, UserKey, INTERAL_SUFFIX_LEN},
};

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
    // entry count
    pub entry_count: usize,
    // restart points
    pub restart_points: Vec<u32>,
    // restart interval
    pub restart_interval: usize,
    // compression type
    pub compression_type: CompressionType,
}

impl Block {
    pub fn get_first_key(&self) -> TableKey<Bytes> {
        let mut buf = BytesMut::default();

        let (_shared_size, shared_len) = usize::decode_var(&self.uncompressed_data[..]).unwrap();
        let (non_shared_size, non_shared_len) =
            usize::decode_var(&self.uncompressed_data[shared_len..]).unwrap();
        let (_value_size, value_len) =
            usize::decode_var(&self.uncompressed_data[shared_len + non_shared_len..]).unwrap();

        let start = shared_len + non_shared_len + value_len;
        let end = start + non_shared_size;
        buf.put(&self.uncompressed_data[start..end - INTERAL_SUFFIX_LEN]);

        let seq = (&self.uncompressed_data[end - INTERAL_SUFFIX_LEN..end]).get_u64();
        TableKey::new(UserKey(buf.freeze()), seq)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.uncompressed_data.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.uncompressed_data.len()
    }

    #[inline]
    pub fn encoded_len(&self) -> usize {
        self.uncompressed_data.len() + 4 + self.restart_points.len() * 4 + 4
    }

    // encode the block into the buffer
    // the format is:
    // compressed: | entries * N | N | restart_points * M | M | (M, N are u32 size)
    pub fn encode_into(&self, buf: &mut impl BufMut) {
        buf.put_slice(&self.uncompressed_data);
        buf.put_u32_le(self.entry_count as u32);
        for point in &self.restart_points {
            buf.put_u32_le(*point);
        }
        buf.put_u32_le(self.restart_points.len() as u32);
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(self.len() + self.restart_points.len() * 4 + 4);
        self.encode_into(&mut buf);
        buf.freeze()
    }

    pub fn decode(buf: &[u8]) -> Self {
        let restart_points_len = (&buf[buf.len() - 4..]).get_u32() as usize;
        let restart_points_start = buf.len() - 4 - restart_points_len * 4;
        let restart_points_end = buf.len() - 4;
        let data_end = restart_points_start - 4;
        let restart_points: Vec<u32> = buf[restart_points_start..restart_points_end]
            .chunks_exact(4)
            .map(|mut p| p.get_u32())
            .collect();
        let entry_count = (&buf[buf.len() - 4 - restart_points_len * 4..]).get_u32() as usize;
        let uncompressed_data = Bytes::copy_from_slice(&buf[..data_end]);

        Block {
            uncompressed_data,
            entry_count,
            restart_points,
            restart_interval: 0,
            compression_type: CompressionType::None,
        }
    }
}
