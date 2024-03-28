use std::mem::size_of;

use bytes::{BufMut, BytesMut};

use super::{Block, CompressionType};
use crate::db::format::{key::TableKey, size::EstimateSize, value::TableValue};
use integer_encoding::*;

pub const DEFAULT_BLOCK_SIZE: usize = 4096;
pub const DEFAULT_RESTART_INTERVAL: usize = 16;

// BlockBuilderOptions is used to create a new BlockBuilder.
pub struct BlockBuilderOptions {
    // Reserved bytes size when creating buffer to avoid frequent allocating.
    pub capacity: usize,
    // Compression algorithm.
    pub compression_type: CompressionType,
    // Restart point interval.
    pub restart_interval: usize,
}

impl Default for BlockBuilderOptions {
    fn default() -> Self {
        BlockBuilderOptions {
            capacity: DEFAULT_BLOCK_SIZE,
            compression_type: CompressionType::None,
            restart_interval: DEFAULT_RESTART_INTERVAL,
        }
    }
}

impl BlockBuilderOptions {
    pub fn new(
        capacity: usize,
        compression_type: CompressionType,
        restart_interval: usize,
    ) -> Self {
        BlockBuilderOptions {
            capacity,
            compression_type,
            restart_interval,
        }
    }
}

/// `BlockBuilder` encodes and appends block to a buffer.
pub struct BlockBuilder {
    // Write buffer.
    buf: BytesMut,
    // Count of entries in current block.
    entry_count: usize,
    // Restart points.
    restart_points: Vec<u32>,
    // Restart interval.
    restart_interval: usize,
    // Last key.
    last_key: Vec<u8>,
    // Compression algorithm.
    compression_type: CompressionType,
    // Max block capacity.
    max_block_capacity: usize,
}

impl BlockBuilder {
    pub fn new(options: BlockBuilderOptions) -> Self {
        BlockBuilder {
            buf: BytesMut::with_capacity(options.capacity),
            entry_count: 0,
            restart_points: vec![],
            restart_interval: options.restart_interval,
            last_key: vec![],
            compression_type: options.compression_type,
            max_block_capacity: options.capacity,
        }
    }

    // An entry is a key-value pair encoded in the block.
    // The format is:
    //     shared_bytes: varint32
    //     non_shared_bytes: varint32
    //     value_length: varint32
    //     key_delta: &[non_shared_bytes]
    //     value: &[value_length]
    pub fn add(&mut self, table_key: TableKey<&[u8]>, table_value: TableValue<&[u8]>) -> bool {
        if !self.is_empty() && self.estimated_size() > self.max_block_capacity {
            return false;
        }
        let mut encoded_key = BytesMut::with_capacity(table_key.len());
        table_key.encode_into(&mut encoded_key);
        let mut encoded_value = BytesMut::with_capacity(table_value.encoded_len());
        table_value.encode_into(&mut encoded_value);

        let mut shared = 0;
        if self.entry_count % self.restart_interval == 0 {
            self.restart_points.push(self.buf.len() as u32);
        } else {
            shared = Self::compute_overlap(&self.last_key, &encoded_key);
        }
        let non_shared = encoded_key.len() - shared;
        let diff_key = &encoded_key[shared..];

        self.buf.extend(shared.encode_var_vec());
        self.buf.extend(non_shared.encode_var_vec());
        self.buf.extend(encoded_value.len().encode_var_vec());

        self.buf.put(diff_key);
        self.buf.put(encoded_value.as_ref());

        self.last_key.clear();
        self.last_key.extend_from_slice(&encoded_key);
        self.entry_count += 1;
        true
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    #[inline]
    pub fn last_key(&self) -> &[u8] {
        &self.last_key
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.entry_count = 0;
        self.restart_points.clear();
        self.last_key.clear();
    }

    #[inline]
    pub fn uncompressed_block_size(&self) -> usize {
        self.buf.len() + self.restart_points.len() * size_of::<u32>() + size_of::<u32>()
    }

    pub fn build(self) -> Block {
        Block {
            uncompressed_data: self.buf.freeze(),
            restart_points: self.restart_points,
            restart_interval: self.restart_interval,
            compression_type: self.compression_type,
            entry_count: self.entry_count,
        }
    }

    #[inline]
    fn compute_overlap(last: &[u8], key: &[u8]) -> usize {
        let mut i = 0;
        let min_len = std::cmp::min(last.len(), key.len());
        while i < min_len && last[i] == key[i] {
            i += 1;
        }
        i
    }
}

impl EstimateSize for BlockBuilder {
    fn estimated_heap_size(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::block::iterator::EntryPrefix;
    use crate::db::format::key::{TableKey, UserKey};
    use crate::db::format::value::TableValue;
    use bytes::Bytes;

    pub fn new_table_key(key: &[u8], seq: u64) -> TableKey<&[u8]> {
        TableKey::new(UserKey(key), seq)
    }

    pub fn new_table_value(value: &[u8]) -> TableValue<&[u8]> {
        TableValue::Put(value)
    }

    #[test]
    fn test_block_builder() {
        let mut builder = BlockBuilder::new(BlockBuilderOptions::default());
        builder.add(new_table_key(b"key1", 0), new_table_value(b"value1"));
        builder.add(new_table_key(b"key2", 0), new_table_value(b"value2"));
        builder.add(new_table_key(b"key3", 0), new_table_value(b"value3"));
        assert!(!builder.is_empty());
        assert_eq!(builder.entry_count, 3);
    }

    #[test]
    fn test_block_get_first_key() {
        let mut builder = BlockBuilder::new(BlockBuilderOptions::default());
        builder.add(new_table_key(b"key1", 0), new_table_value(b"value1"));
        builder.add(new_table_key(b"key2", 0), new_table_value(b"value2"));
        builder.add(new_table_key(b"key3", 0), new_table_value(b"value3"));
        assert!(!builder.is_empty());
        let block = builder.build();
        assert_eq!(
            block.get_first_key(),
            TableKey::new(UserKey(Bytes::from("key1")), 0)
        );
    }

    #[test]
    fn test_block_entry_prefix() {
        let mut builder = BlockBuilder::new(BlockBuilderOptions::default());
        builder.add(new_table_key(b"key1", 0), new_table_value(b"value1"));
        builder.add(new_table_key(b"key2", 0), new_table_value(b"value2"));
        builder.add(new_table_key(b"key3", 0), new_table_value(b"value3"));
        assert!(!builder.is_empty());
        let block = builder.build();
        let prefix = EntryPrefix::decode(&block.uncompressed_data, 0);
        assert_eq!(
            block.uncompressed_data[prefix.unshared_key_range()],
            new_table_key(b"key1", 0).encode()
        );
        assert_eq!(
            &block.uncompressed_data[prefix.value_range()],
            new_table_value(b"value1").encode()
        );
        assert_eq!(
            block.get_first_key(),
            TableKey::new(UserKey(Bytes::from("key1")), 0)
        );

        let entry_len = prefix.entry_len();
        let prefix = EntryPrefix::decode(&block.uncompressed_data[entry_len..], entry_len);
        assert_eq!(
            block.uncompressed_data[prefix.unshared_key_range()],
            new_table_key(b"2", 0).encode()
        );
        assert_eq!(
            &block.uncompressed_data[prefix.value_range()],
            new_table_value(b"value2").encode()
        );

        let entry_len = entry_len + prefix.entry_len();
        let prefix = EntryPrefix::decode(&block.uncompressed_data[entry_len..], entry_len);
        assert_eq!(
            block.uncompressed_data[prefix.unshared_key_range()],
            new_table_key(b"3", 0).encode()
        );
        assert_eq!(
            &block.uncompressed_data[prefix.value_range()],
            new_table_value(b"value3").encode()
        );
    }
}
