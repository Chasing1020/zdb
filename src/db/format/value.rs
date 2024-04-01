use bytes::{BufMut, Bytes};

use super::size::EstimateSize;

#[derive(Clone, Default)]
pub enum TableValue<T> {
    Put(T),
    #[default]
    Delete,
}

pub const VALUE_PUT: u8 = 0;
pub const VALUE_DELETE: u8 = 1 << 0;

impl<T> Copy for TableValue<T> where T: Copy {}

impl<T: AsRef<[u8]>> std::fmt::Debug for TableValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableValue::Put(v) => write!(f, "Put: {:?}", v.as_ref()),
            TableValue::Delete => write!(f, "Delete"),
        }
    }
}

impl<T: PartialEq> PartialEq for TableValue<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Put(l), Self::Put(r)) => l.eq(r),
            (Self::Delete, Self::Delete) => true,
            _ => false,
        }
    }
}

impl<T: Eq> Eq for TableValue<T> {}

impl<T> TableValue<T> {
    pub fn is_delete(&self) -> bool {
        matches!(self, TableValue::Delete)
    }

    pub fn is_put(&self) -> bool {
        matches!(self, TableValue::Put(_))
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

impl<T: AsRef<[u8]>> TableValue<T> {
    pub fn as_slice(&self) -> TableValue<&[u8]> {
        match self {
            TableValue::Put(data) => TableValue::Put(data.as_ref()),
            TableValue::Delete => TableValue::Delete,
        }
    }

    pub fn encode_into(&self, buf: &mut impl BufMut) {
        match self {
            TableValue::Put(data) => {
                buf.put_u8(VALUE_PUT);
                buf.put(data.as_ref());
            }
            TableValue::Delete => {
                buf.put_u8(VALUE_DELETE);
            }
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(match self {
            TableValue::Put(data) => 1 + data.as_ref().len(),
            TableValue::Delete => 1,
        });
        self.encode_into(&mut buf);
        buf
    }

    pub fn encoded_len(&self) -> usize {
        match self {
            TableValue::Put(data) => data.as_ref().len() + 1,
            TableValue::Delete => 1,
        }
    }
}

impl<'a> TableValue<&'a [u8]> {
    pub fn decode(buf: &'a [u8]) -> Self {
        match buf[0] {
            VALUE_PUT => TableValue::Put(&buf[1..]),
            VALUE_DELETE => TableValue::Delete,
            _ => unreachable!(),
        }
    }
}

impl EstimateSize for TableValue<Bytes> {
    #[inline]
    fn estimated_heap_size(&self) -> usize {
        match self {
            TableValue::Put(v) => v.estimated_heap_size(),
            TableValue::Delete => 0,
        }
    }
}
