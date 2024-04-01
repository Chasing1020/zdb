pub mod builder;
pub mod iterator;
pub mod prefix;

use std::{
    io::{Read, Write},
    sync::Arc,
};

use bytes::{Buf, BufMut, Bytes, BytesMut};

use self::prefix::EntryPrefix;

use super::{
    error::{StorageError, StorageResult},
    format::key::TableKey,
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

// BlockCache is a cache for block.
// K: (sstable_id, block_id)
pub type BlockCache = moka::future::Cache<(u32, u32), Arc<Block>>;

// Block is the basic unit of storage in the SSTable file.
// It contains the uncompressed data and restart points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    // uncompressed data
    pub uncompressed_data: Bytes,
    // entry count
    pub entry_count: usize,
    // restart points
    pub restart_points: Vec<u32>,
    // restart interval·
    pub restart_interval: usize,
    // compression type
    pub compression_type: CompressionType,
}

impl Block {
    pub fn get_first_key(&self) -> TableKey<&[u8]> {
        let prefix = EntryPrefix::decode(&self.uncompressed_data, 0);
        let key = &self.uncompressed_data[prefix.unshared_key_range()];
        TableKey::decode(key)
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
    pub fn uncompressed_len(&self) -> usize {
        self.uncompressed_data.len() + 4 + self.restart_points.len() * 4 + 4 + 4
    }

    // encode the block into the buffer
    // the format is:
    // compressed: | entries * N | N | restart_points * M | M | restart_interval |(M, N are u32 size)
    // uncompressed: | uncompressed_len | compression_type |
    pub fn encode_into(&self, target: &mut BytesMut) {
        let mut buf = BytesMut::with_capacity(
            self.uncompressed_data.len() + 4 + self.restart_points.len() * 4 + 4 + 4,
        );
        buf.put_slice(&self.uncompressed_data);
        buf.put_u32(self.entry_count as u32);
        for point in &self.restart_points {
            buf.put_u32(*point);
        }
        buf.put_u32(self.restart_points.len() as u32);
        buf.put_u32(self.restart_interval as u32);

        Self::compress(&buf, self.compression_type, target);
        target.put_u32(self.uncompressed_len() as u32);
        target.put_u8(self.compression_type as u8);
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(self.len() + self.restart_points.len() * 4 + 4);
        self.encode_into(&mut buf);
        buf.freeze()
    }

    pub fn decode(buf: &[u8]) -> Self {
        let compression_type = CompressionType::decode(&mut &buf[buf.len() - 1..]).unwrap();
        let uncompressed_len = (&buf[buf.len() - 5..buf.len() - 1]).get_u32() as usize;
        let data = Self::uncompress(&buf[..buf.len() - 5], compression_type, uncompressed_len);

        let restart_interval = (&data[data.len() - 4..]).get_u32() as usize;
        let restart_points_len = (&data[data.len() - 8..]).get_u32() as usize;
        let restart_points_start = data.len() - 8 - restart_points_len * 4;
        let restart_points_end = data.len() - 8;
        let data_end = restart_points_start - 4;
        let restart_points: Vec<u32> = data[restart_points_start..restart_points_end]
            .chunks_exact(4)
            .map(|mut p| p.get_u32())
            .collect();
        let entry_count = (&data[data_end..]).get_u32() as usize;
        let uncompressed_data = Bytes::copy_from_slice(&data[..data_end]);

        Block {
            uncompressed_data,
            entry_count,
            restart_points,
            restart_interval,
            compression_type,
        }
    }

    pub fn compress(buf: &[u8], compression_type: CompressionType, compress_writer: &mut BytesMut) {
        match compression_type {
            CompressionType::Lz4 => {
                let mut encoder = lz4::EncoderBuilder::new()
                    .level(4)
                    .build(compress_writer.writer())
                    .unwrap();
                encoder.write_all(buf).unwrap();
                encoder.finish().1.unwrap();
            }
            CompressionType::Zstd => {
                let mut encoder = zstd::Encoder::new(compress_writer.writer(), 4).unwrap();
                encoder.write_all(buf).unwrap();
                encoder.finish().unwrap();
            }
            CompressionType::Snappy => {
                let mut encoder = snap::write::FrameEncoder::new(compress_writer.writer());
                encoder.write_all(buf).unwrap();
                encoder.flush().unwrap();
            }
            CompressionType::None => {
                compress_writer.put_slice(buf);
            }
        }
    }

    pub fn uncompress(
        buf: &[u8],
        compression_type: CompressionType,
        uncompressed_capacity: usize,
    ) -> Bytes {
        match compression_type {
            CompressionType::Lz4 => {
                let mut decoder = lz4::Decoder::new(buf.reader()).unwrap();
                let mut decoded = Vec::with_capacity(uncompressed_capacity);
                decoder.read_to_end(&mut decoded).unwrap();
                Bytes::from(decoded)
            }
            CompressionType::Zstd => {
                let mut decoder = zstd::Decoder::new(buf.reader()).unwrap();
                let mut decoded = Vec::with_capacity(uncompressed_capacity);
                decoder.read_to_end(&mut decoded).unwrap();
                Bytes::from(decoded)
            }
            CompressionType::Snappy => {
                let mut decoder = snap::read::FrameDecoder::new(buf);
                let mut decoded = Vec::with_capacity(uncompressed_capacity);
                decoder.read_to_end(&mut decoded).unwrap();
                Bytes::from(decoded)
            }
            CompressionType::None => Bytes::copy_from_slice(buf),
        }
    }
}
