pub mod builder;
pub mod filter;
use std::path::Path;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use self::filter::Bloom;

use super::error::StorageResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMeta {
    // Block offset in sstable.
    pub offset: u32,
    // Block len.
    pub len: u32,
    // Block entries.
    pub entries: u32,
    // First table key.
    pub first_key: Bytes,
    // Last table key.
    pub last_key: Bytes,
}

impl BlockMeta {
    pub fn new(offset: u32, len: u32, entries: u32, first_key: Bytes, last_key: Bytes) -> Self {
        BlockMeta {
            offset,
            len,
            entries,
            first_key,
            last_key,
        }
    }

    pub fn encode_into(&self, buf: &mut impl BufMut) {
        buf.put_u32(self.offset);
        buf.put_u32(self.len);
        buf.put_u32(self.entries);
        buf.put_u32(self.first_key.len() as u32);
        buf.put_slice(&self.first_key);
        buf.put_u32(self.last_key.len() as u32);
        buf.put_slice(&self.last_key);
    }

    pub fn decode_from(buf: &mut impl Buf) -> Self {
        let offset = buf.get_u32();
        let len = buf.get_u32();
        let entries = buf.get_u32();
        let first_key_len = buf.get_u32() as usize;
        let first_key = buf.copy_to_bytes(first_key_len);
        let last_key_len = buf.get_u32() as usize;
        let last_key = buf.copy_to_bytes(last_key_len);
        BlockMeta {
            offset,
            len,
            entries,
            first_key,
            last_key,
        }
    }

    pub fn encode_many(block_metas: &[BlockMeta], buf: &mut impl BufMut) {
        buf.put_u32(block_metas.len() as u32);
        block_metas
            .iter()
            .for_each(|block_meta| block_meta.encode_into(buf));
    }

    pub fn decode_many(buf: &mut impl Buf) -> Vec<BlockMeta> {
        let mut block_metas = vec![];
        let len = buf.get_u32();
        (0..len).for_each(|_| block_metas.push(BlockMeta::decode_from(buf)));
        block_metas
    }
}

pub struct Sstable {
    // Async file.
    pub file: File,
    // Block metas.
    pub block_metas: Vec<BlockMeta>,
    // block meta offset.
    pub block_meta_offset: u64,
    // bloomfilter offset.
    pub bloomfilter_offset: u64,
    // bloomfilter.
    pub bloom_filter: Option<Bloom>,
}

impl Sstable {
    pub fn new(
        file: File,
        block_metas: Vec<BlockMeta>,
        block_meta_offset: u64,
        bloomfilter_offset: u64,
        bloom_filter: Option<Bloom>,
    ) -> Self {
        Sstable {
            file,
            block_metas,
            block_meta_offset,
            bloomfilter_offset,
            bloom_filter,
        }
    }

    pub async fn open(path: impl AsRef<Path>) -> StorageResult<Sstable> {
        let mut file = tokio::fs::File::open(path).await?;
        let len = file.metadata().await?.len() as usize;
        let mut buf = BytesMut::with_capacity(len);
        file.read_buf(&mut buf).await?;

        let bloomfilter_offset = (&buf[buf.len() - 4..]).get_u32();
        let filter = Bloom::decode(&buf[bloomfilter_offset as usize..buf.len() - 4]);

        let block_meta_offset = (&buf[bloomfilter_offset as usize - 4..]).get_u32() as u64;

        let block_metas = BlockMeta::decode_many(
            &mut &buf[block_meta_offset as usize..bloomfilter_offset as usize - 4],
        );
        let bloomfilter_offset = buf.get_u32_le() as u64;
        Ok(Sstable::new(
            file,
            block_metas,
            block_meta_offset,
            bloomfilter_offset,
            Some(filter),
        ))
    }
}
