use std::sync::Arc;

use super::{
    block::builder::{DEFAULT_BLOCK_CACHE_CAPACITY, DEFAULT_BLOCK_SIZE},
    memtable::MemTable,
    sstable::{builder::DEFAULT_SSTABLE_SIZE, Sstable},
};

pub const IMM_MEM_TABLE_LIMIT: usize = 10;

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
    /// The sstables in each level.
    pub levels: Vec<LevelMeta>,
}

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

impl Default for StorageState {
    fn default() -> Self {
        StorageState {
            mem_table: MemTable::new(0),
            imm_mem_tables: Vec::new(),
            levels: Vec::new(),
        }
    }
}
