use bytes::BytesMut;
use integer_encoding::*;
use std::{ops::Range, sync::Arc};

use crate::db::format::key::TableKey;

use super::Block;

// EntryPrefix represents the prefix of an entry in the block.
pub struct EntryPrefix {
    // only used for get value range
    offset: usize,

    shared_len: usize,
    shared_size: usize,
    non_shared_len: usize,
    non_shared_size: usize,
    value_len: usize,
    value_size: usize,
}

impl EntryPrefix {
    pub fn decode(buf: &[u8], offset: usize) -> Self {
        let (shared_size, shared_len) = usize::decode_var(buf).unwrap();
        let (non_shared_size, non_shared_len) = usize::decode_var(&buf[shared_len..]).unwrap();
        let (value_size, value_len) =
            usize::decode_var(&buf[shared_len + non_shared_len..]).unwrap();
        Self {
            offset,
            shared_len,
            shared_size,
            non_shared_len,
            non_shared_size,
            value_len,
            value_size,
        }
    }

    #[inline]
    pub fn entry_len(&self) -> usize {
        self.shared_len
            + self.non_shared_len
            + self.value_len
            + self.non_shared_size
            + self.value_size
    }

    #[inline]
    pub fn unshared_key_range(&self) -> Range<usize> {
        self.key_start()..self.key_end()
    }

    #[inline]
    pub fn value_range(&self) -> Range<usize> {
        self.value_start()..self.value_end()
    }

    #[inline]
    fn key_start(&self) -> usize {
        self.offset + self.shared_len + self.non_shared_len + self.value_len
    }

    #[inline]
    fn key_end(&self) -> usize {
        self.key_start() + self.non_shared_size
    }

    #[inline]
    fn value_start(&self) -> usize {
        self.key_end()
    }

    #[inline]
    fn value_end(&self) -> usize {
        self.value_start() + self.value_size
    }

    // TODO: remove this function
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.extend(usize::encode_var_vec(self.shared_size));
        buf.extend(usize::encode_var_vec(self.non_shared_size));
        buf.extend(usize::encode_var_vec(self.value_size));
    }
}

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
        TableKey::decode(&self.key[..]).unwrap()
    }

    #[inline]
    pub fn table_value(&self) -> &[u8] {
        &self.block.uncompressed_data[self.value_range.clone()]
    }

    pub fn next(&mut self) {
        if !self.try_next() {
            self.invalidate();
        }
    }

    fn try_next(&mut self) -> bool {
        if self.offset > self.block.len() {
            return false;
        }
        let prefix = EntryPrefix::decode(&self.block.uncompressed_data[self.offset..], self.offset);
        let entry_len = prefix.entry_len();

        if self.offset + entry_len > self.block.len() || self.offset > self.block.len() {
            return false;
        }

        self.offset += entry_len;
        self.key.truncate(prefix.shared_len);
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
        if self.offset == 0 {
            return false;
        }
        let restart_point_idx =
            self.block.restart_points[self.idx % self.block.restart_interval] as usize;
        let old_offset = self.offset;
        self.seek_to_restart_point(restart_point_idx);
        self.next_until_prev_offset(old_offset);
        true
    }

    fn seek_to_restart_point(&mut self, restart_point_idx: usize) {
        self.offset = self.block.restart_points[restart_point_idx] as usize;
        self.idx = restart_point_idx * self.block.restart_interval;
        let prefix = EntryPrefix::decode(&self.block.uncompressed_data[self.offset..], self.offset);
        self.key = BytesMut::from(&self.block.uncompressed_data[prefix.unshared_key_range()]);
        self.value_range = prefix.value_range();
        self.entry_len = prefix.entry_len();
    }

    fn next_until_prev_offset(&mut self, offset: usize) {
        while self.offset + self.entry_len < std::cmp::min(self.block.len(), offset) {
            self.next();
        }
    }

    // TODO: Seek the iterator to the first key that is greater than or equal to the given key.
    // fn next_util_key(&mut self, key: TableKey<&[u8]>) {
    //     while self.is_valid() && self.table_key().cmp(&key) == Ordering::Less {
    //         self.next();
    //     }
    // }
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
        while iter.is_valid() {
            let key = iter.table_key();
            let value = iter.table_value();
            assert_eq!(key.user_key.0, format!("key{}", i).as_bytes());
            assert_eq!(value, format!("value{}", i).as_bytes());
            i += 1;
            iter.next();
        }
    }

    #[test]
    fn test_block_iterator_prev() {
        let mut iter = build_iterator_for_test();
        let mut i = 49;
        while iter.is_valid() {
            let key = iter.table_key();
            let value = iter.table_value();
            assert_eq!(key.user_key.0, format!("key{}", i).as_bytes());
            assert_eq!(value, format!("value{}", i).as_bytes());
            i -= 1;
            iter.prev();
        }
    }
}
