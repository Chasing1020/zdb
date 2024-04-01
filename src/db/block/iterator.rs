use bytes::BytesMut;
use std::{cmp::Ordering, ops::Range, sync::Arc};

use super::{prefix::EntryPrefix, Block};
use crate::db::format::{key::TableKey, value::TableValue};

/// Iterates on a block.
pub struct BlockIterator {
    /// reference to the block
    block: Arc<Block>,
    /// offset of the block
    offset: usize,
    /// the current key at the iterator position
    key: BytesMut,
    /// the value range from the block
    value_range: Range<usize>,
    /// the current index at the iterator position
    idx: usize,
    /// the length of the current entry
    entry_len: usize,
}

impl BlockIterator {
    pub fn new(block: Arc<Block>) -> Self {
        BlockIterator {
            block,
            offset: 0,
            key: BytesMut::default(),
            value_range: 0..0,
            idx: 0,
            entry_len: 0,
        }
    }

    pub fn create_and_seek_to_first(block: Arc<Block>) -> Self {
        let mut iter = Self::new(block);
        iter.seek_to_first();
        iter
    }

    pub fn create_and_seek_to_key(block: Arc<Block>, key: TableKey<&[u8]>) -> Self {
        let mut iter = Self::new(block);
        iter.seek(key);
        iter
    }

    fn invalidate(&mut self) {
        self.key = BytesMut::default();
        self.value_range = 0..0;
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.key.is_empty()
    }

    #[inline]
    pub fn table_key(&self) -> TableKey<&[u8]> {
        TableKey::decode(&self.key[..])
    }

    #[inline]
    pub fn table_value(&self) -> TableValue<&[u8]> {
        TableValue::decode(&self.block.uncompressed_data[self.value_range.clone()])
    }

    pub fn next(&mut self) {
        if !self.try_next() {
            self.invalidate();
        }
    }

    fn try_next(&mut self) -> bool {
        if self.offset > self.block.len() || self.idx >= self.block.entry_count {
            return false;
        }
        let prefix = EntryPrefix::decode(&self.block.uncompressed_data[self.offset..], self.offset);
        let entry_len = prefix.entry_len();

        if self.offset + entry_len > self.block.len() {
            return false;
        }

        self.offset += entry_len;
        self.key.truncate(prefix.shared_size);
        self.key
            .extend_from_slice(&self.block.uncompressed_data[prefix.unshared_key_range()]);
        self.value_range = prefix.value_range();
        self.idx += 1;
        self.entry_len = entry_len;
        true
    }

    pub fn prev(&mut self) {
        if !self.try_prev() {
            self.invalidate();
        }
    }

    fn try_prev(&mut self) -> bool {
        if self.offset == 0 || self.idx == 0 {
            return false;
        }
        let mut restart_point_idx = self.idx / self.block.restart_interval;
        if restart_point_idx != 0 && self.idx % self.block.restart_interval == 0 {
            restart_point_idx -= 1;
        }

        let old_offset = self.offset - self.entry_len;
        self.seek_to_restart_point(restart_point_idx);
        self.next_until_prev_offset(old_offset);
        self.idx -= 1;
        true
    }

    fn seek_to_restart_point(&mut self, restart_point_idx: usize) {
        self.offset = self.block.restart_points[restart_point_idx] as usize;
        self.idx = restart_point_idx * self.block.restart_interval + 1;
        let prefix = EntryPrefix::decode(&self.block.uncompressed_data[self.offset..], self.offset);
        self.key = BytesMut::from(&self.block.uncompressed_data[prefix.unshared_key_range()]);
        self.value_range = prefix.value_range();
        self.entry_len = prefix.entry_len();
        self.offset += self.entry_len;
    }

    pub fn seek_to_first(&mut self) {
        self.seek_to_restart_point(0);
    }

    pub fn seek_to_last(&mut self) {
        self.seek_to_restart_point(self.block.restart_points.len() - 1);
        self.next_until_prev_offset(self.block.len());
    }

    fn next_until_prev_offset(&mut self, offset: usize) {
        while self.offset < self.block.len().min(offset) {
            self.next();
        }
    }

    pub fn seek(&mut self, key: TableKey<&[u8]>) {
        self.seek_restart_point_by_key(key);
        self.next_until_key(key);
    }

    pub fn seek_le(&mut self, key: TableKey<&[u8]>) {
        self.seek_restart_point_by_key(key);
        self.next_until_key(key);
        if !self.is_valid() {
            self.seek_to_last();
        }
        self.prev_until_key(key);
    }

    fn seek_restart_point_by_key(&mut self, key: TableKey<&[u8]>) {
        let index = self.search_restart_point_index_by_key(key);
        self.seek_to_restart_point(index)
    }

    // Time complexity: O(logN)
    fn search_restart_point_index_by_key(&self, key: TableKey<&[u8]>) -> usize {
        let res = self
            .block
            .restart_points
            .partition_point(|&restart_point| {
                let prefix = EntryPrefix::decode(
                    &self.block.uncompressed_data[restart_point as usize..],
                    restart_point as usize,
                );
                match TableKey::decode(&self.block.uncompressed_data[prefix.unshared_key_range()])
                    .cmp(&key)
                {
                    Ordering::Less | Ordering::Equal => true,
                    Ordering::Greater => false,
                }
            })
            .saturating_sub(1);
        res
    }

    fn next_until_key(&mut self, key: TableKey<&[u8]>) {
        while self.is_valid() && self.table_key().cmp(&key) == Ordering::Less {
            self.next();
        }
    }

    fn prev_until_key(&mut self, key: TableKey<&[u8]>) {
        while self.is_valid() && self.table_key().cmp(&key) == Ordering::Greater {
            self.prev();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::db::{
        block::builder::{BlockBuilder, BlockBuilderOptions},
        format::{
            key::{TableKey, UserKey},
            value::TableValue,
        },
    };

    use super::BlockIterator;

    fn new_table_key(key: &[u8], seq: u64) -> TableKey<&[u8]> {
        TableKey::new(UserKey(key), seq)
    }

    fn new_table_value(value: &[u8]) -> TableValue<&[u8]> {
        TableValue::Put(value)
    }

    fn build_iterator_for_test() -> BlockIterator {
        let options = BlockBuilderOptions::default();
        let mut builder = BlockBuilder::new(options);
        for i in 0..50 {
            let k = format!("key{}", i);
            let v = format!("value{}", i);
            builder.add(
                new_table_key(k.as_bytes(), 0),
                new_table_value(v.as_bytes()),
            );
        }
        let block = builder.build();
        BlockIterator::new(Arc::new(block))
    }

    #[test]
    fn test_block_iterator_next() {
        let mut iter = build_iterator_for_test();
        let mut i = 0;
        iter.next();
        while iter.is_valid() {
            let key = iter.table_key();
            let value = iter.table_value();
            assert_eq!(key.user_key.0, format!("key{}", i).as_bytes());
            assert_eq!(value, TableValue::put(format!("value{}", i).as_bytes()));
            i += 1;
            iter.next();
        }
    }

    #[test]
    fn test_block_iterator_prev() {
        let mut iter = build_iterator_for_test();
        let mut i = 49;
        iter.seek_to_last();
        while iter.is_valid() {
            let key = iter.table_key();
            let value = iter.table_value();
            assert_eq!(key.user_key.0, format!("key{}", i).as_bytes());
            assert_eq!(value, TableValue::put(format!("value{}", i).as_bytes()));
            i -= 1;
            iter.prev();
        }
    }

    #[test]
    fn test_block_iterator_seek() {
        let mut iter = build_iterator_for_test();
        let key = new_table_key(b"key25", 0);
        iter.seek(key);
        assert_eq!(iter.table_key().user_key.0, b"key25");

        let key = new_table_key(b"key100", 0);
        iter.seek(key);
        assert_eq!(iter.table_key().user_key.0, b"key2");

        let key = new_table_key(b"key100", 0);
        iter.seek_le(key);
        assert_eq!(iter.table_key().user_key.0, b"key1");
    }
}
