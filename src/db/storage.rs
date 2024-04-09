use std::sync::Arc;

use parking_lot::RwLock;

use super::{
    block::builder::{DEFAULT_BLOCK_CACHE_CAPACITY, DEFAULT_BLOCK_SIZE},
    memtable::MemTable,
    mvcc::MvccManager,
    sstable::{builder::DEFAULT_SSTABLE_SIZE, Sstable},
};

pub const IMM_MEM_TABLE_LIMIT: usize = 10;

#[derive(Debug, Clone)]
pub struct StorageOptions {
    /// Block size.
    pub block_size: usize,
    /// Block cache capacity.
    pub block_cache_capacity: usize,
    /// SST size, which is the target size of SSTable.
    pub target_sst_size: usize,
    // Maximum immutable number of memtables in memory.
    pub imm_memtable_limit: usize,
}

impl Default for StorageOptions {
    fn default() -> Self {
        StorageOptions {
            block_size: DEFAULT_BLOCK_SIZE,
            block_cache_capacity: DEFAULT_BLOCK_CACHE_CAPACITY,
            target_sst_size: DEFAULT_SSTABLE_SIZE,
            imm_memtable_limit: IMM_MEM_TABLE_LIMIT,
        }
    }
}

/// The meta of a level.
pub struct LevelMeta {
    /// The level number from 0 to max_level.
    pub level: u32,
    /// The sstables in this level.
    pub tables: Vec<Arc<Sstable>>,
}

/// The storage state.
pub struct StorageState {
    /// The memtable.
    pub mem_table: MemTable,
    /// The immutable memtables.
    pub imm_mem_tables: Vec<MemTable>,
    /// The sstables in each level, from level 0 to max_level.
    pub levels: Vec<LevelMeta>,
}

impl Default for StorageState {
    fn default() -> Self {
        StorageState {
            mem_table: MemTable::new(0),
            imm_mem_tables: Vec::new(),
            levels: Vec::new(),
        }
    }
}

pub struct StorageManager {
    pub state: Arc<RwLock<StorageState>>,
    /// The MVCC manager.
    pub mvcc_manager: Arc<MvccManager>,
}

impl StorageManager {
    // pub fn get_with_ts(&self, key: &[u8], ts: u64) -> Option<Bytes> {
    //     // let state = self.state.read();

    //     todo!()
    // }

    #[inline]
    pub fn mvcc_manager(&self) -> &MvccManager {
        self.mvcc_manager.as_ref()
    }
}
