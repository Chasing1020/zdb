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
    // First table key.
    pub first_key: Bytes,
    // Last table key.
    pub last_key: Bytes,
}

impl BlockMeta {
    pub fn new(offset: u64, len: u64, entries: u32, first_key: Bytes, last_key: Bytes) -> Self {
        BlockMeta {
            offset,
            len,
            entry_count: entries,
            first_key,
            last_key,
        }
    }

    pub fn encode_into(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.offset);
        buf.put_u64_le(self.len);
        buf.put_u32_le(self.entry_count);
        buf.put_u32_le(self.first_key.len() as u32);
        buf.put_slice(&self.first_key);
        buf.put_u32_le(self.last_key.len() as u32);
        buf.put_slice(&self.last_key);
    }

    pub fn decode_from(buf: &mut impl Buf) -> Self {
        let offset = buf.get_u64_le();
        let len = buf.get_u64_le();
        let entries = buf.get_u32_le();
        let first_key_len = buf.get_u32_le() as usize;
        let first_key = buf.copy_to_bytes(first_key_len);
        let last_key_len = buf.get_u32_le() as usize;
        let last_key = buf.copy_to_bytes(last_key_len);
        BlockMeta {
            offset,
            len,
            entry_count: entries,
            first_key,
            last_key,
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
    // bloomfilter offset.
    pub bloomfilter_offset: u64,
}

impl Sstable {
    pub fn new(
        file: File,
        block_metas: Vec<BlockMeta>,
        block_meta_offset: u64,
        bloomfilter_offset: u64,
    ) -> Self {
        Sstable {
            file,
            block_metas,
            block_meta_offset,
            bloomfilter_offset,
        }
    }
}
