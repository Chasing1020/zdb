use std::sync::Arc;

use bytes::Bytes;

use crate::db::format::{key::TableKey, value::TableValue};

use super::Block;

/// Iterates on a block.
pub struct BlockIterator {
    /// reference to the block
    block: Arc<Block>,
    /// the current key at the iterator position
    key: TableKey<Bytes>,
    /// the value range from the block
    value_range: (usize, usize),
    /// the current index at the iterator position
    idx: usize,
    /// the first key in the block
    first_key: TableKey<Bytes>,
}

impl BlockIterator {
    pub fn new(block: Arc<Block>) -> Self {
        let first_key = block.get_first_key();
        BlockIterator {
            block,
            key: TableKey::default(),
            value_range: (0, 0),
            idx: 0,
            first_key,
        }
    }
}
