use bytes::Bytes;

use crate::db::{
    error::StorageResult,
    format::{key::TableKey, value::TableValue},
};

pub trait ReadView {
    /// `first_rev` returns the first KV revision at the time of opening the txn.
    /// After a compaction, the first revision increases to the compaction revision.
    fn first_rev(&self) -> u64;

    /// `rev` returns the revision of the KV at the time of opening the txn.
    fn rev(&self) -> u64;
}

pub trait ReadableTxn: ReadView {
    /// `get` gets the value for the key.
    fn get(&self, key: &[u8]) -> StorageResult<Option<Bytes>>;
    // TODO: `scan` scans the key-value pairs with the given range.
}

pub trait WriteView: ReadView {
    /// `delete` deletes the key.
    fn delete(&self, key: &[u8]);
    /// `put` sets the value for the key.
    fn put(&self, key: &[u8], value: &[u8]);
}

pub trait WritableTxn: ReadableTxn + WriteView {
    /// `commit` commits the transaction.
    fn commit(&self) -> StorageResult<()>;
    /// `changes` gets the changes made since opening the write txn.
    fn changes(&self) -> Vec<(TableKey<Bytes>, TableValue<Bytes>)>;
}
