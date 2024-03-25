use super::format::{key::MemTableKey, size::EstimateSize, value::MemTableValue};

use bytes::Bytes;
use crossbeam_skiplist::SkipMap;
use std::sync::{atomic::AtomicUsize, Arc};

#[derive(Clone)]
pub struct MemTable {
    id: usize,
    pub(crate) buffer: Arc<SkipMap<MemTableKey<Bytes>, MemTableValue<Bytes>>>,
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
        key: MemTableKey<Bytes>,
        value: MemTableValue<Bytes>,
    ) -> Result<(), anyhow::Error> {
        let estimated_size = key.estimated_size() + value.estimated_size();
        self.kv_size
            .fetch_add(estimated_size, std::sync::atomic::Ordering::Relaxed);
        self.buffer.insert(key, value);
        Ok(())
    }

    pub fn get(&self, key: &MemTableKey<Bytes>) -> Option<MemTableValue<Bytes>> {
        self.buffer.get(key).map(|e| e.value().clone())
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use crate::db::format::key::UserKey;

    use super::*;

    #[test]
    fn test_memtable() {
        let memtable = MemTable::new(0);
        // gen test
        let key = MemTableKey::new(UserKey(Bytes::from("key".as_bytes())), 0);
        let value = MemTableValue::Put(Bytes::from("value".as_bytes()));
        memtable.insert(key.clone(), value.clone()).unwrap();

        assert_eq!(memtable.get(&key).unwrap(), value);
    }
}
