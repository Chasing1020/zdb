use crate::db::{error::StorageResult, storage::StorageState};

use super::picker::{CompactionTask, Picker};

// Level compaction style is the default compaction style of RocksDB,
// so it is also the most widely used compaction style among users.
// ref: https://github.com/facebook/rocksdb/wiki/Choose-Level-Compaction-Files
#[derive(Debug, Clone)]
pub struct LeveledCompactionOptions {
    // TODO: Maximum number of compactions allowed.
    pub max_background_compactions: usize,

    pub level_size_multiplier: usize,
    // Compaction triggers when number of L0 files reaches `level0_file_num_compaction_trigger`,
    // files of L0 will be merged into L1.
    pub level0_file_num_compaction_trigger: usize,
    pub max_levels: usize,
    pub max_bytes_for_level_base: u64,
}

pub struct LeveledCompactionTask {
    pub source_level: usize,
    pub source_sst_ids: Vec<u32>,
    pub target_level: usize,
    pub target_sst_ids: Vec<u32>,
}

#[async_trait::async_trait]
impl CompactionTask for LeveledCompactionTask {
    async fn run(&mut self) -> StorageResult<()> {
        unimplemented!()
    }
}

pub struct LeveledCompactionPicker {
    options: LeveledCompactionOptions,
}

impl LeveledCompactionPicker {
    pub fn new(options: LeveledCompactionOptions) -> Self {
        LeveledCompactionPicker { options }
    }
}

impl Picker for LeveledCompactionPicker {
    fn pick(&self, state: &StorageState) -> Option<Box<dyn super::picker::CompactionTask>> {
        let mut target_level_size = vec![0; self.options.max_levels];
        let mut real_level_size = vec![0; self.options.max_levels];

        // Calculate the real size of each level.
        state.levels.iter().enumerate().for_each(|(i, level)| {
            let level_size = level.tables.iter().map(|t| t.table_size()).sum::<u64>();
            real_level_size[i] = level_size;
        });

        // Target size of the last level (num_levels-1) will be actual size of largest level (which is usually the last level).
        // And then Target_Size(Ln-1) = Target_Size(Ln) / max_bytes_for_level_multiplier
        // We won't fill any level whose target will be lower than
        // `max_bytes_for_level_base` / `max_bytes_for_level_multiplier`.
        // These levels will be kept empty and all L0 compaction will skip
        // those levels and directly go to the first level with valid target size.
        let mut base_level = self.options.max_levels;
        target_level_size[self.options.max_levels - 1] =
            real_level_size[self.options.max_levels - 1].max(self.options.max_bytes_for_level_base);
        (0..self.options.max_levels - 1).rev().for_each(|i| {
            target_level_size[i] =
                target_level_size[i + 1] / self.options.level_size_multiplier as u64;
            if target_level_size[i] < self.options.max_bytes_for_level_base {
                target_level_size[i] = 0;
            }
            if target_level_size[i] > 0 {
                base_level = i + 1;
            }
        });

        if state.levels[0].tables.len() >= self.options.level0_file_num_compaction_trigger {
            return Some(Box::new(LeveledCompactionTask {
                source_level: 0,
                source_sst_ids: state.levels[0].tables.iter().map(|t| t.id()).collect(),
                target_level: base_level,
                target_sst_ids: vec![], // TODO: to check overlap
            }));
        }

        todo!()
    }
}
