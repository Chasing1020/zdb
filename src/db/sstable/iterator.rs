use std::sync::Arc;

use crate::db::{
    block::iterator::BlockIterator,
    error::StorageResult,
    format::{key::TableKey, value::TableValue},
};

use super::Sstable;

// The iterator of SSTable.
pub struct SstableIterator {
    pub(crate) table: Arc<Sstable>,
    block_iterator: BlockIterator,
    block_idx: usize,
}

impl SstableIterator {
    async fn seek_to_first_inner(table: &Arc<Sstable>) -> StorageResult<(usize, BlockIterator)> {
        Ok((0, BlockIterator::new(table.get_block(0).await?)))
    }

    pub async fn create_and_seek_to_first(table: Arc<Sstable>) -> StorageResult<Self> {
        let (block_idx, block_iterator) = Self::seek_to_first_inner(&table).await?;
        let iter = Self {
            block_iterator,
            table,
            block_idx,
        };
        Ok(iter)
    }

    pub async fn seek_to_first(&mut self) -> StorageResult<()> {
        (self.block_idx, self.block_iterator) = Self::seek_to_first_inner(&self.table).await?;
        Ok(())
    }

    pub async fn create_and_seek_to_key(
        table: Arc<Sstable>,
        key: TableKey<&[u8]>,
    ) -> StorageResult<Self> {
        let (block_idx, block_iterator) = Self::seek_to_key_inner(&table, key).await?;
        let iter = Self {
            table,
            block_iterator,
            block_idx,
        };
        Ok(iter)
    }

    async fn seek_to_key_inner(
        table: &Arc<Sstable>,
        key: TableKey<&[u8]>,
    ) -> StorageResult<(usize, BlockIterator)> {
        let mut blk_idx = table.get_block_idx(key);
        let mut block_iterator =
            BlockIterator::create_and_seek_to_key(table.get_block(blk_idx as u32).await?, key);
        if !block_iterator.is_valid() {
            blk_idx += 1;
            if blk_idx < table.block_num() {
                block_iterator = BlockIterator::new(table.get_block(blk_idx as u32).await?);
            }
        }
        Ok((blk_idx, block_iterator))
    }

    pub async fn seek_to_key(&mut self, key: TableKey<&[u8]>) -> StorageResult<()> {
        (self.block_idx, self.block_iterator) = Self::seek_to_key_inner(&self.table, key).await?;
        Ok(())
    }
}

impl SstableIterator {
    pub fn value(&self) -> TableValue<&[u8]> {
        self.block_iterator.table_value()
    }

    pub fn key(&self) -> TableKey<&[u8]> {
        self.block_iterator.table_key()
    }

    pub fn is_valid(&self) -> bool {
        self.block_iterator.is_valid()
    }

    pub async fn next(&mut self) -> StorageResult<()> {
        self.block_iterator.next();
        if !self.block_iterator.is_valid() {
            self.block_idx += 1;
            if self.block_idx < self.table.block_num() {
                self.block_iterator = BlockIterator::create_and_seek_to_first(
                    self.table.get_block(self.block_idx as u32).await?,
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path;

    use crate::db::{
        format::key::UserKey,
        sstable::builder::{SstableBuilder, SstableBuilderOptions},
    };

    use super::*;

    pub fn new_table_key(key: &[u8], seq: u64) -> TableKey<&[u8]> {
        TableKey::new(UserKey(key), seq)
    }

    pub fn new_table_value(value: &[u8]) -> TableValue<&[u8]> {
        TableValue::Put(value)
    }

    async fn build_sstable_iterator_for_test() -> SstableIterator {
        let mut builder = SstableBuilder::new(SstableBuilderOptions::default());
        for i in 0..1000 {
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
        SstableIterator::create_and_seek_to_first(Arc::new(sstable))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_sstable_iterator_next() {
        let mut iter = build_sstable_iterator_for_test().await;
        for i in 0..1000 {
            iter.next().await.unwrap();
            assert_eq!(iter.key().user_key.0, format!("key{}", i).as_bytes());
            assert_eq!(
                iter.value(),
                TableValue::Put(format!("value{}", i).as_bytes())
            );
        }
        iter.next().await.unwrap();
        assert!(!iter.is_valid());
    }

    #[tokio::test]
    async fn test_sstable_iterator_seek() {
        let mut iter = build_sstable_iterator_for_test().await;
        iter.seek_to_key(new_table_key(format!("key{}", 500).as_bytes(), 0))
            .await
            .unwrap();
        assert_eq!(iter.key().user_key.0, format!("key{}", 500).as_bytes());
        assert_eq!(
            iter.value(),
            TableValue::Put(format!("value{}", 500).as_bytes())
        );
    }
}
