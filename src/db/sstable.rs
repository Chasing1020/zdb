pub mod builder;

use std::fs::File;

use bytes::{Buf, BufMut, Bytes};

pub struct BlockMeta {
    // Block offset in sstable.
    pub offset: u64,
    // Block len.
    pub len: u64,
    // Block entries.
    pub entry_count: u32,
    // First table key
    pub first_key: Bytes,
}

impl BlockMeta {
    pub fn new(offset: u64, len: u64, entries: u32, first_key: Bytes) -> Self {
        BlockMeta {
            offset,
            len,
            entry_count: entries,
            first_key,
        }
    }

    pub fn encode_into(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.offset);
        buf.put_u64_le(self.len);
        buf.put_u32_le(self.entry_count);
        buf.put_u32_le(self.first_key.len() as u32);
        buf.put_slice(&self.first_key);
    }

    pub fn decode_from(buf: &mut impl Buf) -> Self {
        let offset = buf.get_u64_le();
        let len = buf.get_u64_le();
        let entries = buf.get_u32_le();
        let key_len = buf.get_u32_le() as usize;
        let first_key = buf.copy_to_bytes(key_len);
        BlockMeta {
            offset,
            len,
            entry_count: entries,
            first_key,
        }
    }
}

pub struct Sstable {
    // File path.
    pub file: File,
    // Block metas.
    pub block_metas: Vec<BlockMeta>,
    // block meta offset.
    pub block_meta_offset: u64,
}

impl Sstable {
    pub fn new(file: File, block_metas: Vec<BlockMeta>, block_meta_offset: u64) -> Self {
        Sstable {
            file,
            block_metas,
            block_meta_offset,
        }
    }
}