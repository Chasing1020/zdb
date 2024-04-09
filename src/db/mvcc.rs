use std::{
    collections::BTreeMap,
    sync::{atomic::AtomicU64, Arc},
};

use bytes::Bytes;
use parking_lot::RwLock;

use self::transaction::Transaction;

use super::storage::StorageManager;

pub mod transaction;
pub mod view;

/// `ReaderCounter` maintains the lowest read timestamp among all in-progress transactions.
#[derive(Default)]
pub struct ReaderCounter {
    readers: BTreeMap<u64, usize>, // read_ts -> count
}

impl ReaderCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_reader(&mut self, ts: u64) {
        *self.readers.entry(ts).or_default() += 1;
    }

    pub fn remove_reader(&mut self, ts: u64) {
        let cnt = self.readers.get_mut(&ts).unwrap();
        *cnt -= 1;
        if *cnt == 0 {
            self.readers.remove(&ts);
        }
    }

    pub fn num_retained_snapshots(&self) -> usize {
        self.readers.len()
    }

    pub fn min_read_ts(&self) -> u64 {
        self.readers
            .first_key_value()
            .map(|(ts, _)| *ts)
            .unwrap_or(0)
    }
}

pub struct CommittedTxnData {
    pub kvs: Vec<(Bytes, Bytes)>,
    pub read_ts: u64,
    pub commit_ts: u64,
}

/// `MvccManager` is responsible for generating timestamps and managing transactions.
pub struct MvccManager {
    /// `curr_ts` is the latest commited timestamp. It updates when any txn commits.
    curr_ts: AtomicU64,
    /// `reader_counter` is the lowest read timestamp among all in-progress transactions.
    reader_counter: Arc<RwLock<ReaderCounter>>,
    /// `commited_txn_data` stores current committed transaction data.
    pub commited_txn_data: Arc<RwLock<BTreeMap<u64, CommittedTxnData>>>,
}

impl MvccManager {
    pub fn new(init_ts: u64) -> Self {
        Self {
            curr_ts: AtomicU64::new(init_ts),
            reader_counter: Arc::new(RwLock::new(ReaderCounter::new())),
            commited_txn_data: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    #[inline]
    pub fn curr_ts(&self) -> u64 {
        self.curr_ts.load(std::sync::atomic::Ordering::Acquire)
    }

    /// `update_curr_ts` updates the latest commited timestamp.
    #[inline]
    pub fn update_curr_ts(&self, ts: u64) {
        self.curr_ts.store(ts, std::sync::atomic::Ordering::Release);
    }

    #[inline]
    pub fn min_read_ts(&self) -> u64 {
        self.reader_counter.read().min_read_ts()
    }

    pub fn new_txn(&self, storage: Arc<StorageManager>) -> Transaction {
        let curr_ts = self.curr_ts();
        self.reader_counter.write().add_reader(curr_ts);
        Transaction::new(storage, curr_ts)
    }
}
