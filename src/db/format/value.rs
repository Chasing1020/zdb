use bytes::Bytes;

use super::size::EstimateSize;

#[derive(Clone)]
pub enum MemTableValue<T> {
    Put(T),
    Delete,
}

impl<T> Copy for MemTableValue<T> where T: Copy {}

impl<T: AsRef<[u8]>> std::fmt::Debug for MemTableValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemTableValue::Put(v) => write!(f, "Put: {:?}", v.as_ref()),
            MemTableValue::Delete => write!(f, "Delete"),
        }
    }
}

impl<T: PartialEq> PartialEq for MemTableValue<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Put(l), Self::Put(r)) => l.eq(r),
            (Self::Delete, Self::Delete) => true,
            _ => false,
        }
    }
}

impl<T: Eq> Eq for MemTableValue<T> {}

impl<T> MemTableValue<T> {
    pub fn is_delete(&self) -> bool {
        matches!(self, MemTableValue::Delete)
    }

    pub fn is_put(&self) -> bool {
        matches!(self, MemTableValue::Put(_))
    }

    pub fn into_user_value(self) -> Option<T> {
        match self {
            Self::Put(val) => Some(val),
            Self::Delete => None,
        }
    }

    pub fn put(data: T) -> Self {
        Self::Put(data)
    }

    pub fn delete() -> Self {
        Self::Delete
    }
}

impl<T: AsRef<[u8]>> MemTableValue<T> {
    pub fn as_slice(&self) -> MemTableValue<&[u8]> {
        match self {
            MemTableValue::Put(data) => MemTableValue::Put(data.as_ref()),
            MemTableValue::Delete => MemTableValue::Delete,
        }
    }
}

impl EstimateSize for MemTableValue<Bytes> {
    #[inline]
    fn estimated_heap_size(&self) -> usize {
        match self {
            MemTableValue::Put(v) => v.estimated_heap_size(),
            MemTableValue::Delete => 0,
        }
    }
}
