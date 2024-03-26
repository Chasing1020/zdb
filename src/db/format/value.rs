use bytes::{BufMut, Bytes};

use super::size::EstimateSize;

#[derive(Clone, Default)]
pub enum MemTableValue<T> {
    Put(T),
    #[default]
    Delete,
}

pub const VALUE_PUT: u8 = 0;
pub const VALUE_DELETE: u8 = 1 << 0;

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

    pub fn encode_into(&self, buf: &mut impl BufMut) {
        match self {
            MemTableValue::Put(data) => {
                buf.put_u8(VALUE_PUT);
                buf.put(data.as_ref());
            }
            MemTableValue::Delete => {
                buf.put_u8(VALUE_DELETE);
            }
        }
    }

    pub fn encoded_len(&self) -> usize {
        match self {
            MemTableValue::Put(data) => data.as_ref().len() + 1,
            MemTableValue::Delete => 1,
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
