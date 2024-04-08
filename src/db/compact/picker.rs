use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};

use crate::db::{error::StorageResult, storage::StorageState};

#[async_trait::async_trait]
pub trait CompactionTask {
    async fn run(&mut self, state: &mut RwLockWriteGuard<'_, StorageState>) -> StorageResult<()>;
}

/// Picker picks input SST files and builds the compaction task.
/// Different compaction strategy may implement different pickers.
pub trait Picker {
    fn pick(&self, state: RwLockReadGuard<'_, StorageState>) -> Option<Box<dyn CompactionTask>>;
}
