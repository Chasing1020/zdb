use std::sync::Arc;

use crate::db::{
    error::StorageResult,
    sstable::{iterator::SstableIterator, Sstable},
};

use super::{iterator::StorageIterator, key::TableKey, value::TableValue};

// Concat multiple sstables into one iterator.
// the Tables' key range must be consecutive and non-overlapping.
pub struct SstConcatIterator {
    current: Option<SstableIterator>,
    next_sst_idx: usize,
    sstables: Vec<Arc<Sstable>>,
}

impl SstConcatIterator {
    pub fn can_concat(sstables: Vec<Arc<Sstable>>) -> bool {
        for i in 1..sstables.len() {
            if sstables[i - 1].last_key >= sstables[i].first_key {
                return false;
            }
        }
        true
    }

    pub async fn create_and_seek_to_key(
        sstables: Vec<Arc<Sstable>>,
        key: TableKey<&[u8]>,
    ) -> StorageResult<Self> {
        debug_assert!(Self::can_concat(sstables.clone()));
        let idx: usize = sstables
            .partition_point(|table| table.first_key().as_slice_key() <= key)
            .saturating_sub(1);
        if idx >= sstables.len() {
            return Ok(Self {
                current: None,
                next_sst_idx: sstables.len(),
                sstables,
            });
        }
        let mut iter = Self {
            current: Some(
                SstableIterator::create_and_seek_to_key(sstables[idx].clone(), key).await?,
            ),
            next_sst_idx: idx + 1,
            sstables,
        };
        iter.move_until_valid().await?;
        Ok(iter)
    }

    async fn move_until_valid(&mut self) -> StorageResult<()> {
        while let Some(iter) = self.current.as_mut() {
            if iter.is_valid() {
                break;
            }
            if self.next_sst_idx >= self.sstables.len() {
                self.current = None;
            } else {
                self.current = Some(
                    SstableIterator::create_and_seek_to_first(
                        self.sstables[self.next_sst_idx].clone(),
                    )
                    .await?,
                );
                self.next_sst_idx += 1;
            }
        }
        Ok(())
    }
}

impl StorageIterator for SstConcatIterator {
    type KeyType<'a> = TableKey<&'a [u8]>;
    type ValueType<'a> = TableValue<&'a [u8]>;

    fn value(&self) -> TableValue<&[u8]> {
        self.current.as_ref().unwrap().value()
    }

    fn key(&self) -> TableKey<&[u8]> {
        self.current.as_ref().unwrap().key()
    }

    fn is_valid(&self) -> bool {
        match self.current {
            Some(ref iter) => iter.is_valid(),
            None => false,
        }
    }

    async fn next(&mut self) -> StorageResult<()> {
        if let Some(iter) = self.current.as_mut() {
            iter.next().await?;
            self.move_until_valid().await?;
        }
        Ok(())
    }
}
