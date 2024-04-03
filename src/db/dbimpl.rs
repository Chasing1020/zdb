use std::{os::macos::raw::stat, path::{Path, PathBuf}, sync::{Arc, RwLock}};

use super::{block::BlockCache, storage::{StorageOptions, StorageState}};

pub struct DBImpl {
    pub(crate) path: PathBuf,
    pub(crate) state: Arc<RwLock<StorageState>>,
    pub(crate) block_cache: Arc<BlockCache>,
    pub(crate) options: Arc<StorageOptions>,
}

impl DBImpl {
    pub fn new(path: impl AsRef<Path>, options: StorageOptions) -> Self {
        let options = Arc::new(options);
        let path = path.as_ref().to_path_buf();
        let state = Arc::new(RwLock::new(StorageState::default()));
        let block_cache = Arc::new(BlockCache::new(options.block_cache_capacity as u64));
        
        DBImpl {
            path,
            state,
            block_cache,
            options,
        }
    }
}
