use crate::db::error::StorageResult;

pub trait StorageIterator {
    type KeyType<'a>: PartialEq + Eq + PartialOrd + Ord
    where
        Self: 'a;

    type ValueType<'a>: PartialEq + Eq
    where
        Self: 'a;

    /// Get the current value.
    fn value(&self) -> Self::ValueType<'_>;

    /// Get the current key.
    fn key(&self) -> Self::KeyType<'_>;

    /// Check if the current iterator is valid.
    fn is_valid(&self) -> bool;

    /// Move to the next position.
    fn next(&mut self) -> impl std::future::Future<Output = StorageResult<()>>;

    /// Number of underlying active iterators for this iterator.
    fn num_active_iterators(&self) -> usize {
        1
    }
}
