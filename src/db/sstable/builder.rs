use std::{fs::File, io::Write, path::Path, sync::Arc};

use super::{filter::BloomBuilder, BlockMeta, Sstable};
use crate::db::{
    block::{
        builder::{
            BlockBuilder, BlockBuilderOptions, DEFAULT_BLOCK_SIZE, DEFAULT_RESTART_INTERVAL,
        },
        BlockCache, CompressionType,
    },
    error::StorageResult,
    format::{key::TableKey, size::EstimateSize, value::TableValue},
};

use bytes::{BufMut, Bytes, BytesMut};

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
    // Block builder.
    block_builder: BlockBuilder,
    // Block metas.
    block_metas: Vec<BlockMeta>,
    // Bloom filter.
    bloom_builder: BloomBuilder,
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
            block_builder,
            block_metas: vec![],
            bloom_builder: BloomBuilder::new(options.false_positive_rate),
        }
    }

    pub fn add(&mut self, table_key: TableKey<&[u8]>, table_value: TableValue<&[u8]>) {
        self.bloom_builder.add(table_key.encode());

        if self.block_builder.add(table_key, table_value) {
            return;
        }

        self.finish_block();

        assert!(self.block_builder.add(table_key, table_value));
    }

    fn finish_block(&mut self) {
        self.block_count += 1;
        let last_key = TableKey::from_bytes(Bytes::copy_from_slice(self.block_builder.last_key()));
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
            block.uncompressed_len() as u32,
            block.entry_count as u32,
            block.get_first_key().to_bytes(),
            last_key,
        ));
    }

    // Build the sstable file.
    // The format is:
    // | block1 | block2 | ... | blockN | N | block_metas | block_meta_offset | bloomfilter | bloomfilter_offset |
    pub async fn build(
        mut self,
        id: u32,
        path: impl AsRef<Path>,
        block_cache: Option<Arc<BlockCache>>,
    ) -> StorageResult<Sstable> {
        self.finish_block();
        self.buf.put_u32(self.block_count);

        let block_meta_offset = self.buf.len() as u32;
        BlockMeta::encode_many(&self.block_metas, &mut self.buf);
        self.buf.put_u32(block_meta_offset);

        let bloomfilter_offset = self.buf.len() as u32;
        let bloom_filter = self.bloom_builder.build();
        bloom_filter.encode_into(&mut self.buf);
        self.buf.put_u32(bloomfilter_offset);

        let mut file = File::create(path)?;
        file.write_all(&self.buf)?;

        let sstable = Sstable::new(
            id,
            file,
            self.block_metas,
            block_meta_offset,
            bloomfilter_offset,
            Some(bloom_filter),
            block_cache,
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
        let sstable = builder
            .build(0, path::Path::new("test.sst"), None)
            .await
            .unwrap();
        assert_eq!(sstable.block_metas.len(), 1);
        assert_eq!(sstable.block_metas[0].entries, 50);
        std::fs::remove_file("test.sst").unwrap();
    }

    #[tokio::test]
    async fn test_sstable_build_and_open() {
        let mut builder = SstableBuilder::new(SstableBuilderOptions::default());
        let keys = 10000;
        for i in 0..keys {
            let k = format!("key{}", i);
            let v = format!("value{}", i);
            builder.add(
                new_table_key(k.as_bytes(), 0),
                new_table_value(v.as_bytes()),
            );
        }
        let sstable = builder
            .build(0, path::Path::new("test.sst"), None)
            .await
            .unwrap();

        let sstable_from_file = Sstable::open(0, path::Path::new("test.sst"), None)
            .await
            .unwrap();
        assert_eq!(sstable.block_metas, sstable_from_file.block_metas);
        assert_eq!(sstable.bloom_filter, sstable_from_file.bloom_filter);
        std::fs::remove_file("test.sst").unwrap();
    }
}
