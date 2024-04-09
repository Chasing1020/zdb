use std::collections::{BTreeMap, HashSet};

use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};

use crate::db::{
    error::StorageResult,
    format::{concat_iterator::SstConcatIterator, two_merge_iterator::TwoMergeIterator},
    storage::StorageState,
};

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
    pub source_ids: Vec<u32>,
    pub target_level: usize,
    pub target_ids: Vec<u32>,
}

#[async_trait::async_trait]
impl CompactionTask for LeveledCompactionTask {
    async fn run(&mut self, state: &mut RwLockWriteGuard<'_, StorageState>) -> StorageResult<()> {
        // let mut files_to_remove = vec![];
        let source_id_set = self.source_ids.iter().copied().collect::<HashSet<_>>();
        let target_id_set = self.target_ids.iter().copied().collect::<HashSet<_>>();
        let source_tables = state.levels[self.source_level]
            .tables
            .iter()
            .filter(|table| source_id_set.contains(&table.id()))
            .cloned()
            .collect::<Vec<_>>();
        let target_tables = state.levels[self.target_level]
            .tables
            .iter()
            .filter(|table| target_id_set.contains(&table.id()))
            .cloned()
            .collect::<Vec<_>>();
        let source_table_iter = SstConcatIterator::create_and_seek_to_first(source_tables).await?;
        let target_table_iter = SstConcatIterator::create_and_seek_to_first(target_tables).await?;

        let _iter = TwoMergeIterator::create(source_table_iter, target_table_iter);
        // TODO: Generate new SSTable from iter with new key sequence.

        state.levels[self.source_level]
            .tables
            .retain(|table| !source_id_set.contains(&table.id()));
        state.levels[self.target_level]
            .tables
            .retain(|table| !target_id_set.contains(&table.id()));

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

    pub fn find_overlapped_target_ids(
        state: RwLockReadGuard<'_, StorageState>,
        source_level: usize,
        source_ids: &[u32],
        target_level: usize,
    ) -> Vec<u32> {
        let source_ssts = state.levels[source_level]
            .tables
            .iter()
            .filter(|t| source_ids.contains(&t.id()))
            .collect::<Vec<_>>();
        let begin_key = source_ssts
            .iter()
            .map(|t| t.first_key())
            .min()
            .unwrap()
            .clone();
        let end_key = source_ssts
            .iter()
            .map(|t| t.last_key())
            .max()
            .unwrap()
            .clone();
        state.levels[target_level]
            .tables
            .iter()
            .filter_map(|t| {
                if t.first_key() <= &begin_key && t.last_key() >= &end_key {
                    Some(t.id())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Picker for LeveledCompactionPicker {
    fn pick(
        &self,
        state: RwLockReadGuard<'_, StorageState>,
    ) -> Option<Box<dyn super::picker::CompactionTask>> {
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

        // flush L0 to base level
        if state.levels[0].tables.len() >= self.options.level0_file_num_compaction_trigger {
            let source_ids = state.levels[0]
                .tables
                .iter()
                .map(|t| t.id())
                .collect::<Vec<_>>();
            let target_ids = Self::find_overlapped_target_ids(state, 0, &source_ids, base_level);
            return Some(Box::new(LeveledCompactionTask {
                source_level: 0,
                source_ids,
                target_level: base_level,
                target_ids,
            }));
        }

        let mut scores = BTreeMap::new();
        (1..self.options.max_levels).for_each(|i| {
            let score = real_level_size[i] as f64 / target_level_size[i] as f64;
            if score > 1.0 {
                scores.insert(i, score);
            }
        });

        // Pick the level with the highest score.
        let level = scores.iter().next().map(|(i, _)| *i)?;
        let source_id = state.levels[level + 1]
            .tables
            .iter()
            .map(|table| table.id())
            .max()?;

        Some(Box::new(LeveledCompactionTask {
            source_level: level,
            source_ids: vec![source_id],
            target_level: level + 1,
            target_ids: Self::find_overlapped_target_ids(state, level, &[source_id], level + 1),
        }))
    }
}
