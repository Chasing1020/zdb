pub mod builder;
pub mod file;
pub mod filter;
pub mod iterator;

use std::path::Path;
use std::sync::Arc;

use bytes::{Buf, BufMut, Bytes};

use self::file::FileObject;
use self::filter::Bloom;

use super::block::{Block, BlockCache};
use super::error::{StorageError, StorageResult};
use super::format::key::TableKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMeta {
    // Block offset in sstable.
    pub offset: u32,
    // Block len.
    pub len: u32,
    // Block entries.
    pub entries: u32,
    // First table key.
    pub first_key: TableKey<Bytes>,
    // Last table key.
    pub last_key: TableKey<Bytes>,
}

impl BlockMeta {
    pub fn new(
        offset: u32,
        len: u32,
        entries: u32,
        first_key: TableKey<Bytes>,
        last_key: TableKey<Bytes>,
    ) -> Self {
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
        self.first_key.encode_into(buf);
        buf.put_u32(self.last_key.len() as u32);
        self.last_key.encode_into(buf);
    }

    pub fn decode_from(buf: &mut impl Buf) -> Self {
        let offset = buf.get_u32();
        let len = buf.get_u32();
        let entries = buf.get_u32();
        let first_key_len = buf.get_u32() as usize;
        let first_key = TableKey::from_bytes(buf.copy_to_bytes(first_key_len));
        let last_key_len = buf.get_u32() as usize;
        let last_key = TableKey::from_bytes(buf.copy_to_bytes(last_key_len));

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
    // Sstable id.
    pub id: u32,
    // Async file.
    pub file: FileObject,
    // Block metas.
    pub block_metas: Vec<BlockMeta>,
    // block meta offset.
    pub block_meta_offset: u32,
    // bloomfilter offset.
    pub bloomfilter_offset: u32,
    // bloomfilter.
    pub bloom_filter: Option<Bloom>,
    // block cache.
    pub block_cache: Option<Arc<BlockCache>>,
}

impl Sstable {
    pub fn new(
        id: u32,
        file: FileObject,
        block_metas: Vec<BlockMeta>,
        block_meta_offset: u32,
        bloomfilter_offset: u32,
        bloom_filter: Option<Bloom>,
        block_cache: Option<Arc<BlockCache>>,
    ) -> Self {
        Sstable {
            id,
            file,
            block_metas,
            block_meta_offset,
            bloomfilter_offset,
            bloom_filter,
            block_cache,
        }
    }

    pub async fn open(
        id: u32,
        path: impl AsRef<Path>,
        block_cache: Option<Arc<BlockCache>>,
    ) -> StorageResult<Sstable> {
        let file = FileObject::open(&path)?;
        let len = file.size();
        let mut buf = vec![0; len as usize];
        file.read_to(&mut buf, 0)?;

        let bloomfilter_offset = (&buf[buf.len() - 4..]).get_u32();
        let bloom_filter = Some(Bloom::decode(
            &buf[bloomfilter_offset as usize..buf.len() - 4],
        ));

        let block_meta_offset = (&buf[bloomfilter_offset as usize - 4..]).get_u32();

        let block_metas = BlockMeta::decode_many(
            &mut &buf[block_meta_offset as usize..bloomfilter_offset as usize - 4],
        );
        Ok(Sstable::new(
            id,
            file,
            block_metas,
            block_meta_offset,
            bloomfilter_offset,
            bloom_filter,
            block_cache,
        ))
    }

    pub fn get_block_idx(&self, key: TableKey<&[u8]>) -> usize {
        self.block_metas
            .partition_point(|block_meta| block_meta.first_key.as_ref() <= key)
            .saturating_sub(1)
    }

    pub async fn get_block(&self, block_id: u32) -> StorageResult<Arc<Block>> {
        if let Some(cache) = &self.block_cache {
            cache
                .try_get_with((self.id, block_id), self.get_block_by_buf(block_id))
                .await
                .map_err(|_| StorageError::BlockNotFoundError(block_id))
        } else {
            self.get_block_by_buf(block_id).await
        }
    }

    async fn get_block_by_buf(&self, block_id: u32) -> StorageResult<Arc<Block>> {
        let block_meta = self
            .block_metas
            .get(block_id as usize)
            .ok_or_else(|| StorageError::BlockNotFoundError(block_id))?;
        let mut buf = vec![0; block_meta.len as usize];
        // TODO: read_exact_at is not async, need to be fixed?
        self.file.read_to(&mut buf, block_meta.offset as u64)?;

        Ok(Arc::new(Block::decode(&buf)))
    }

    #[inline]
    pub fn block_num(&self) -> usize {
        self.block_metas.len()
    }
}
