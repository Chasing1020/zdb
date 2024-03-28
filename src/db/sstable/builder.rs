use std::{fs::File, hash::Hasher, io::Write, path::Path};

use bloomfilter::Bloom;
use bytes::{BufMut, Bytes, BytesMut};

use crate::db::{
    block::{
        builder::{
            BlockBuilder, BlockBuilderOptions, DEFAULT_BLOCK_SIZE, DEFAULT_RESTART_INTERVAL,
        },
        CompressionType,
    },
    error::StorageResult,
    format::{key::TableKey, size::EstimateSize, value::TableValue},
};

use super::{BlockMeta, Sstable};

pub const DEFAULT_SSTABLE_SIZE: usize = 4 * 1024 * 1024; // 4MB
pub const DEFAULT_BLOOM_FILTER_FALSE_POSITIVE_RATE: f64 = 0.01;

#[derive(Debug, Clone)]
pub struct SstableBuilderOptions {
    // sstable capacity
    pub capacity: usize,
    // block capacity
    pub block_capacity: usize,
    // bloom filter false positive rate
    pub false_positive_rate: f64,
    // Compression algorithm.
    pub compression_type: CompressionType,
    // Restart point interval.
    pub restart_interval: usize,
}

impl Default for SstableBuilderOptions {
    fn default() -> Self {
        SstableBuilderOptions {
            capacity: DEFAULT_SSTABLE_SIZE,
            block_capacity: DEFAULT_BLOCK_SIZE,
            false_positive_rate: DEFAULT_BLOOM_FILTER_FALSE_POSITIVE_RATE,
            compression_type: CompressionType::None,
            restart_interval: DEFAULT_RESTART_INTERVAL,
        }
    }
}

pub struct SstableBuilder {
    // Builder options.
    options: SstableBuilderOptions,
    // Write buffer.
    buf: BytesMut,
    // Block count.
    block_count: u32,
    // First key.
    first_key: Vec<u8>,
    // Block builder.
    block_builder: BlockBuilder,
    // Block metas.
    block_metas: Vec<BlockMeta>,
    // Bloom filter.
    bloom_filter: Bloom<u64>,
}

impl SstableBuilder {
    pub fn new(options: SstableBuilderOptions) -> Self {
        let block_builder = BlockBuilder::new(BlockBuilderOptions::new(
            options.block_capacity,
            options.compression_type,
            options.restart_interval,
        ));
        SstableBuilder {
            options: options.clone(),
            buf: BytesMut::with_capacity(options.capacity),
            block_count: 0,
            first_key: vec![],
            block_builder,
            block_metas: vec![],
            bloom_filter: Bloom::new_for_fp_rate(options.capacity, options.false_positive_rate),
        }
    }

    pub fn add(&mut self, table_key: TableKey<&[u8]>, table_value: TableValue<&[u8]>) {
        if self.first_key.is_empty() {
            self.first_key = table_key.encode().to_vec();
        }

        let mut hasher = ahash::AHasher::default();
        hasher.write(&table_key.encode());
        let hash = hasher.finish();
        self.bloom_filter.set(&hash);

        if self.block_builder.add(table_key, table_value) {
            return;
        }

        self.finish_block();

        assert!(self.block_builder.add(table_key, table_value));
        self.first_key = table_key.encode().to_vec();
    }

    fn finish_block(&mut self) {
        self.block_count += 1;
        let last_key = Bytes::copy_from_slice(self.block_builder.last_key());
        let new_block_builder = BlockBuilder::new(BlockBuilderOptions::new(
            self.options.block_capacity,
            self.options.compression_type,
            self.options.restart_interval,
        ));
        let builder = std::mem::replace(&mut self.block_builder, new_block_builder);

        let block = builder.build();
        let last_offset = self
            .block_metas
            .last()
            .map_or(0, |meta| meta.offset + meta.len);

        block.encode_into(&mut self.buf);

        self.block_metas.push(BlockMeta::new(
            last_offset,
            block.uncompressed_len() as u64,
            block.entry_count as u32,
            Bytes::from(block.get_first_key().encode()),
            last_key,
        ));
    }

    // Build the sstable file.
    // The format is:
    // | block1 | block2 | ... | blockN | N | block_meta_offset | block_metas | bloomfilter_offset | bloomfilter |
    pub async fn build(mut self, path: impl AsRef<Path>) -> StorageResult<Sstable> {
        self.finish_block();
        self.buf.put_u32(self.block_count);

        let block_meta_offset = self.buf.len();
        self.buf.put_u32_le(block_meta_offset as u32);
        self.block_metas
            .iter()
            .for_each(|meta| meta.encode_into(&mut self.buf));

        let filter = self.bloom_filter.bitmap();
        let bloomfilter_offset = self.buf.len();
        self.buf.put(filter.as_slice());

        let mut file = File::create(path)?;
        file.write_all(&self.buf)?;

        let sstable = Sstable::new(
            file,
            self.block_metas,
            block_meta_offset as u64,
            bloomfilter_offset as u64,
        );
        Ok(sstable)
    }
}

impl EstimateSize for SstableBuilder {
    fn estimated_heap_size(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod test {
    use std::path;

    use super::*;
    use crate::db::format::key::{TableKey, UserKey};
    use crate::db::format::value::TableValue;

    pub fn new_table_key(key: &[u8], seq: u64) -> TableKey<&[u8]> {
        TableKey::new(UserKey(key), seq)
    }

    pub fn new_table_value(value: &[u8]) -> TableValue<&[u8]> {
        TableValue::Put(value)
    }

    #[tokio::test]
    async fn test_sstable_builder() {
        let mut builder = SstableBuilder::new(SstableBuilderOptions::default());
        for i in 0..50 {
            let k = format!("key{}", i);
            let v = format!("value{}", i);
            builder.add(
                new_table_key(k.as_bytes(), 0),
                new_table_value(v.as_bytes()),
            );
        }
        let sstable = builder.build(path::Path::new("test.sst")).await.unwrap();
        assert_eq!(sstable.block_metas.len(), 1);
        assert_eq!(sstable.block_metas[0].entry_count, 50);
        std::fs::remove_file("test.sst").unwrap();
    }
}
