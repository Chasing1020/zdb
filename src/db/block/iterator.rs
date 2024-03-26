use bytes::{Bytes, BytesMut};
use integer_encoding::*;
use std::{ops::Range, sync::Arc};

use crate::db::format::key::TableKey;

use super::Block;

/// Iterates on a block.
pub struct BlockIterator {
    /// reference to the block
    block: Arc<Block>,
    /// offset of the block
    offset: usize,
    /// the current key at the iterator position
    key: TableKey<Bytes>,
    /// the value range from the block
    value_range: Range<usize>,
    /// the current index at the iterator position
    idx: usize,
    /// the first key in the block
    first_key: TableKey<Bytes>,
}

// EntryPrefix represents the prefix of an entry in the block.
pub struct EntryPrefix {
    // only used for get value range
    pub offset: usize,

    pub shared_len: usize,
    pub shared_size: usize,
    pub non_shared_len: usize,
    pub non_shared_size: usize,
    pub value_len: usize,
    pub value_size: usize,
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

impl BlockIterator {
    pub fn new(block: Arc<Block>) -> Self {
        let first_key = block.get_first_key();
        BlockIterator {
            block,
            offset: 0,
            key: TableKey::default(),
            value_range: 0..0,
            idx: 0,
            first_key,
        }
    }

    fn invalidate(&mut self) {
        self.key = TableKey::default();
        self.value_range = 0..0;
    }

    fn next(&mut self) {
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

        if self.offset + entry_len > self.block.uncompressed_data.len() {
            return false;
        }
        false
    }
}
