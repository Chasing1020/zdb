use super::{
    error::StorageResult,
    format::{
        iterator::StorageIterator,
        key::{TableKey, TableKeyRange},
        size::EstimateSize,
        value::TableValue,
    },
};

use bytes::Bytes;
use crossbeam_skiplist::{map::Entry, SkipMap};
use ouroboros::self_referencing;
use std::sync::{atomic::AtomicUsize, Arc};

#[derive(Clone)]
pub struct MemTable {
    id: usize,
    pub(crate) buffer: Arc<SkipMap<TableKey<Bytes>, TableValue<Bytes>>>,
    pub(crate) kv_size: Arc<AtomicUsize>,
}

impl MemTable {
    pub fn new(id: usize) -> Self {
        MemTable {
            id,
            buffer: Arc::new(SkipMap::new()),
            kv_size: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn insert(
        &self,
        key: TableKey<Bytes>,
        value: TableValue<Bytes>,
    ) -> Result<(), anyhow::Error> {
        let estimated_size = key.estimated_size() + value.estimated_size();
        self.kv_size
            .fetch_add(estimated_size, std::sync::atomic::Ordering::Relaxed);
        self.buffer.insert(key, value);
        Ok(())
    }

    pub fn get(&self, key: &TableKey<Bytes>) -> Option<TableValue<Bytes>> {
        self.buffer.get(key).map(|e| e.value().clone())
    }

    /// Get an iterator over a range of keys.
    pub fn scan(&self, range: TableKeyRange) -> MemTableIterator {
        let mut iter = MemTableIteratorBuilder {
            map: self.buffer.clone(),
            iter_builder: |map| map.range(range),
            item: (TableKey::default(), TableValue::default()),
        }
        .build();
        let entry = iter.with_iter_mut(|iter| MemTableIterator::entry_to_item(iter.next()));
        iter.with_mut(|x| *x.item = entry);
        iter
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

type SkipMapRangeIter<'a> = crossbeam_skiplist::map::Range<
    'a,
    TableKey<Bytes>,
    TableKeyRange,
    TableKey<Bytes>,
    TableValue<Bytes>,
>;

#[self_referencing]
pub struct MemTableIterator {
    /// Stores a reference to the skipmap.
    map: Arc<SkipMap<TableKey<Bytes>, TableValue<Bytes>>>,
    /// Stores a skipmap iterator that refers to the lifetime of `MemTableIterator` itself.
    #[borrows(map)]
    #[not_covariant]
    iter: SkipMapRangeIter<'this>,
    /// Stores the current key-value pair.
    item: (TableKey<Bytes>, TableValue<Bytes>),
}

impl MemTableIterator {
    fn entry_to_item(
        entry: Option<Entry<'_, TableKey<Bytes>, TableValue<Bytes>>>,
    ) -> (TableKey<Bytes>, TableValue<Bytes>) {
        entry
            .map(|x| (x.key().clone(), x.value().clone()))
            .unwrap_or_else(|| (TableKey::default(), TableValue::default()))
    }
}

impl StorageIterator for MemTableIterator {
    type KeyType<'a> = TableKey<Bytes>;
    type ValueType<'a> = TableValue<Bytes>;

    fn value(&self) -> Self::ValueType<'_> {
        self.borrow_item().1.clone()
    }

    fn key(&self) -> Self::KeyType<'_> {
        self.borrow_item().0.clone()
    }

    fn is_valid(&self) -> bool {
        !self.borrow_item().0.is_empty()
    }

    async fn next(&mut self) -> StorageResult<()> {
        let entry = self.with_iter_mut(|iter| Self::entry_to_item(iter.next()));
        self.with_mut(|x| *x.item = entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Bound;

    use crate::db::format::key::UserKey;

    use super::*;

    #[test]
    fn test_memtable() {
        let memtable = MemTable::new(0);
        // gen test
        let key = TableKey::new(UserKey(Bytes::from("key".as_bytes())), 0);
        let value = TableValue::Put(Bytes::from("value".as_bytes()));
        memtable.insert(key.clone(), value.clone()).unwrap();

        assert_eq!(memtable.get(&key).unwrap(), value);
    }

    #[tokio::test]
    async fn test_memtable_iterator() {
        let memtable = MemTable::new(0);
        // gen test
        let key1 = TableKey::new(UserKey(Bytes::from("key1".as_bytes())), 0);
        let value1 = TableValue::Put(Bytes::from("value1".as_bytes()));
        memtable.insert(key1.clone(), value1.clone()).unwrap();

        let key2 = TableKey::new(UserKey(Bytes::from("key2".as_bytes())), 0);
        let value2 = TableValue::Put(Bytes::from("value2".as_bytes()));
        memtable.insert(key2.clone(), value2.clone()).unwrap();

        let mut iter = memtable.scan((Bound::Unbounded, Bound::Unbounded));

        assert_eq!(iter.key(), key1);
        assert_eq!(iter.value(), value1);
        assert!(iter.is_valid());

        iter.next().await.unwrap();
        assert_eq!(iter.key(), key2);
        assert_eq!(iter.value(), value2);
        assert!(iter.is_valid());

        iter.next().await.unwrap();
        assert!(!iter.is_valid());
    }
}
