use std::{
    borrow::Borrow,
    fmt::{Debug, Display, Formatter},
    mem,
    ops::{Deref, DerefMut},
};

use bytes::{BufMut, Bytes};

use super::size::EstimateSize;

/// A UserKey is the actual key supplied by the calling application, without any internal
/// decorations.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct UserKey<T: AsRef<[u8]>>(pub T);

impl<T: AsRef<[u8]>> Debug for UserKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "UserKey: {:?}", self.0.as_ref())
    }
}

impl<T: AsRef<[u8]>> Display for UserKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "UserKey: {:?}", self.0.as_ref())
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for UserKey<T> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>> Deref for UserKey<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: AsRef<[u8]>> DerefMut for UserKey<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: AsRef<[u8]>> Borrow<[u8]> for UserKey<T> {
    fn borrow(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>> UserKey<T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.0.as_ref().len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_empty()
    }

    #[inline]
    pub fn encode_into(&self, buf: &mut impl BufMut) {
        buf.put(self.0.as_ref());
    }
}

impl EstimateSize for UserKey<Bytes> {
    #[inline]
    fn estimated_heap_size(&self) -> usize {
        self.0.estimated_heap_size()
    }
}

type SeqNumber = u64;
pub const MAX_SEQUENCE_NUMBER: u64 = SeqNumber::MAX;
pub const INTERAL_SUFFIX_LEN: usize = mem::size_of::<SeqNumber>();

/// A MemTableKey is a key in the memtable. It consists of a UserKey and a sequence number.
/// MemTableKey = InternalKey + SeqNumber(8B)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MemTableKey<T: AsRef<[u8]>> {
    pub user_key: UserKey<T>,
    pub seq: SeqNumber,
}

impl<T: AsRef<[u8]>> Debug for MemTableKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MemTableKey: {:?}, Seq: {:?}",
            self.user_key.as_ref(),
            self.seq
        )
    }
}

impl<T: AsRef<[u8]>> Display for MemTableKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemTableKey: {:?}", self.user_key.as_ref())
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for MemTableKey<T> {
    fn as_ref(&self) -> &[u8] {
        self.user_key.as_ref()
    }
}

impl<T: AsRef<[u8]>> MemTableKey<T> {
    pub fn new(user_key: UserKey<T>, seq: SeqNumber) -> MemTableKey<T> {
        Self { user_key, seq }
    }

    /// Encode in to a buffer.
    pub fn encode_into(&self, buf: &mut impl BufMut) {
        self.user_key.encode_into(buf);
        buf.put_u64(self.seq);
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(INTERAL_SUFFIX_LEN + self.user_key.len());
        self.encode_into(&mut buf);
        buf
    }
}

impl EstimateSize for MemTableKey<Bytes> {
    #[inline]
    fn estimated_heap_size(&self) -> usize {
        self.user_key.estimated_heap_size() + mem::size_of::<SeqNumber>()
    }
}
