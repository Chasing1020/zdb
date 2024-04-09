use std::sync::{
    atomic::{AtomicBool, Ordering::SeqCst},
    Arc,
};

use crate::db::{error::StorageResult, storage::StorageManager};

/// `Transaction` represents a transaction in the MVCC model that can modify the storage
pub struct Transaction {
    /// `start_ts` is the start timestamp of the transaction.
    read_ts: u64,
    /// `storage` is the storage state that the transaction reads and writes.
    storage: Arc<StorageManager>,
    /// `committed` indicates whether the transaction is committed.
    committed: AtomicBool,
}

impl Drop for Transaction {
    /// `drop` make sure to remove the reader from the watermark when the transaction is dropped.
    fn drop(&mut self) {
        self.storage
            .mvcc_manager()
            .reader_counter
            .write()
            .remove_reader(self.read_ts);
    }
}

impl Transaction {
    pub fn new(storage: Arc<StorageManager>, read_ts: u64) -> Self {
        storage
            .mvcc_manager()
            .reader_counter
            .write()
            .add_reader(read_ts);
        Self {
            read_ts,
            storage,
            committed: AtomicBool::new(false),
        }
    }

    pub fn commit(&self) -> StorageResult<()> {
        self.committed
            .compare_exchange(false, true, SeqCst, SeqCst)
            .expect("commit should be called only once");

        todo!()
    }
}
