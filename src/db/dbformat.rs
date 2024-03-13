use num_enum::FromPrimitive;
use std::{
    borrow::Borrow,
    cmp::Ordering::{Equal, Less},
    convert::AsRef,
    fmt::{Debug, Display, Formatter},
    io::Write,
    mem,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use bytes::{buf::Buf, BufMut};

use crate::util::comparator::Comparator;
use integer_encoding::*;

pub const MAX_SEQUENCE_NUMBER: u64 = (0x1u64 << 56) - 1;
pub const INTERAL_SUFFIX_LEN: usize = mem::size_of::<u64>();

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, FromPrimitive)]
pub enum ValueType {
    Deletion = 0x0,
    #[default]
    Value = 0x1,
}

/// A UserKey is the actual key supplied by the calling application, without any internal
/// decorations.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
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
}

pub struct DefaultComparator;

impl Comparator for DefaultComparator {
    fn cmp(&self, a: &[u8], b: &[u8]) -> std::cmp::Ordering {
        a.cmp(b)
    }

    // Find the shortest string x, such that a < x < b
    fn find_shortest_separator(&self, a: &[u8], b: &[u8]) -> Vec<u8> {
        let min_length = std::cmp::min(a.len(), b.len());
        let mut diff_index = 0;
        while diff_index < min_length && a[diff_index] == b[diff_index] {
            diff_index += 1;
        }
        let mut tmp = a.to_vec();
        if diff_index < min_length {
            let diff_byte = a[diff_index];
            if diff_byte < 0xff && diff_byte + 1 < b[diff_index] {
                tmp[diff_index] = diff_byte + 1;
                tmp = tmp[..diff_index + 1].to_vec();
            }
        }
        tmp
    }

    fn find_short_successor(&self, key: &[u8]) -> Vec<u8> {
        let mut tmp = key.to_vec();
        for i in 0..tmp.len() {
            if key[i] != u8::max_value() {
                tmp[i] += 1;
                tmp.truncate(i + 1);
                return tmp;
            }
        }
        tmp
    }
}

/// An InternalKey consists of [key, tag], so it's basically a MemtableKey without the initial
/// length specification. This type is used as item type of MemtableIterator, and as the key
/// type of tables.
/// InteralKey = UserKey + SeqNumber(7B) + Type(1B)
#[derive(PartialEq, Eq, Hash, Default)]
pub struct InternalKey<T: AsRef<[u8]>>(pub T);

impl<T: AsRef<[u8]>> Display for InternalKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InternalKey: {:?} {:?} {:?}",
            self.user_key(),
            self.seq(),
            self.vt()
        )
    }
}

impl<T: AsRef<[u8]>> Debug for InternalKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InternalKey: {:?} {:?} {:?}",
            self.user_key(),
            self.seq(),
            self.vt()
        )
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for InternalKey<T> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]>> InternalKey<T> {
    #[inline]
    pub fn user_key(&self) -> &[u8] {
        self.as_ref()[..self.len() - INTERAL_SUFFIX_LEN].as_ref()
    }

    #[inline]
    pub fn seq(&self) -> u64 {
        (&self.as_ref()[self.len() - INTERAL_SUFFIX_LEN..]).get_u64() >> 8
    }

    #[inline]
    pub fn vt(&self) -> ValueType {
        ValueType::from((&self.as_ref()[self.len() - INTERAL_SUFFIX_LEN..]).get_u64() as u8)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.as_ref().len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.user_key().as_ref().is_empty()
    }
}

impl InternalKey<&[u8]> {
    pub fn encode(key: &[u8], seq: u64, vt: ValueType) -> Vec<u8> {
        let mut buf = Vec::with_capacity(key.len() + INTERAL_SUFFIX_LEN);
        buf.write_all(key).expect("write key failed");
        buf.put_u64(seq << 8 | vt as u64);
        buf
    }
}

impl<T: AsRef<[u8]> + Ord + Eq> Ord for InternalKey<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.user_key()
            .cmp(&other.user_key())
            .then(other.seq().cmp(&self.seq()))
    }
}

impl<T: AsRef<[u8]> + Ord + Eq> PartialOrd for InternalKey<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A MemTableKey is a key in the memtable. It consists of a UserKey and a sequence number.
/// MemTableKey = VarInt + InternalKey
/// InternalKey = UserKey + SeqNumber(7B) + Type(1B)
#[derive(PartialEq, Eq, Hash, Default)]
pub struct MemTableKey<T: AsRef<[u8]> + From<Vec<u8>>>(pub T);

impl<T: AsRef<[u8]> + From<Vec<u8>>> AsRef<[u8]> for MemTableKey<T> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<T: AsRef<[u8]> + From<Vec<u8>> + Ord + Eq> From<Vec<u8>> for MemTableKey<T> {
    fn from(v: Vec<u8>) -> Self {
        MemTableKey(T::from(v))
    }
}

impl<T: AsRef<[u8]> + From<Vec<u8>> + Ord + Eq> Ord for MemTableKey<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let (alen, aoff): (usize, usize) = VarInt::decode_var(self.as_ref()).unwrap();
        let (blen, boff): (usize, usize) = VarInt::decode_var(other.as_ref()).unwrap();
        let internal_key_a = InternalKey(&self.as_ref()[aoff..aoff + alen]);
        let internal_key_b = InternalKey(&other.as_ref()[boff..boff + blen]);
        internal_key_a.cmp(&internal_key_b)
    }
}

impl<T: AsRef<[u8]> + From<Vec<u8>> + Ord + Eq> PartialOrd for MemTableKey<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct InternalKeyComparator {
    pub comparator: Rc<Box<dyn Comparator>>,
}

impl InternalKeyComparator {
    pub fn new(comparator: Box<dyn Comparator>) -> Self {
        InternalKeyComparator {
            comparator: Rc::new(comparator),
        }
    }
}

impl Default for InternalKeyComparator {
    fn default() -> Self {
        InternalKeyComparator {
            comparator: Rc::new(Box::new(DefaultComparator)),
        }
    }
}

impl Comparator for InternalKeyComparator {
    fn cmp(&self, a: &[u8], b: &[u8]) -> std::cmp::Ordering {
        let a = InternalKey(a);
        let b = InternalKey(b);
        self.comparator
            .cmp(&a.user_key(), &b.user_key())
            .then(b.seq().cmp(&a.seq()))
    }

    fn find_shortest_separator(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
        let ik_from = InternalKey(from);
        let ik_to = InternalKey(to);

        let sep = self
            .comparator
            .find_shortest_separator(&ik_from.user_key(), &ik_to.user_key());
        if sep.len() < ik_from.user_key().len() && self.comparator.cmp(from, &sep) == Less {
            InternalKey::encode(&sep, MAX_SEQUENCE_NUMBER, ValueType::Value)
        } else {
            InternalKey::encode(ik_from.user_key(), ik_from.seq(), ik_from.vt())
        }
    }

    fn find_short_successor(&self, key: &[u8]) -> Vec<u8> {
        let ik = InternalKey(key);
        let mut tmp = self.comparator.find_short_successor(&ik.user_key());
        if tmp.len() < ik.user_key().len() {
            tmp.put_u64(MAX_SEQUENCE_NUMBER << 8 | ValueType::Value as u64);
            return tmp;
        }
        ik.as_ref().to_vec()
    }
}

// A helper class useful for DBImpl::Get()
// LookupKey = varint32(key_len) + InternalKey
#[derive(Clone, Debug)]
pub struct LookupKey {
    pub key: Vec<u8>,
    pub key_offset: usize,
}

impl LookupKey {
    pub fn new(k: UserKey<&[u8]>, s: u64, t: ValueType) -> LookupKey {
        let mut key = Vec::new();
        let internal_keylen = k.len() + INTERAL_SUFFIX_LEN;
        key.resize(
            k.len() + internal_keylen.required_space() + INTERAL_SUFFIX_LEN,
            0,
        );

        {
            let mut writer = key.as_mut_slice();
            writer
                .write_varint(internal_keylen)
                .expect("write to slice failed");
            writer.write_all(&k).expect("write to slice failed");
            writer
                .write_fixedint(s << 8 | t as u64)
                .expect("write to slice failed");
        }

        LookupKey {
            key,
            key_offset: internal_keylen.required_space(),
        }
    }

    /// Returns the full memtable-formatted key.
    #[inline]
    pub fn memtable_key(self) -> MemTableKey<Vec<u8>> {
        MemTableKey(self.key)
    }

    /// Returns only the user key portion.
    #[inline]
    pub fn user_key(&self) -> UserKey<&[u8]> {
        UserKey(&self.key[self.key_offset..self.key.len() - INTERAL_SUFFIX_LEN])
    }

    /// Returns key and tag.
    #[inline]
    pub fn internal_key(&self) -> InternalKey<&[u8]> {
        InternalKey(&self.key[self.key_offset..])
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    #[test]
    fn test_internal_key_short_separator() {
        let tests = vec![
            vec![
                ("foo", 100, ValueType::Value),
                ("foo", 99, ValueType::Value),
                ("foo", 100, ValueType::Value),
            ],
            vec![
                ("foo", 100, ValueType::Value),
                ("foo", 101, ValueType::Value),
                ("foo", 100, ValueType::Value),
            ],
            vec![
                ("foo", 100, ValueType::Value),
                ("hello", 200, ValueType::Value),
                ("g", MAX_SEQUENCE_NUMBER, ValueType::Value),
            ],
            vec![
                ("foo", 100, ValueType::Value),
                ("foobar", 200, ValueType::Value),
                ("foo", 100, ValueType::Value),
            ],
        ];

        for t in tests {
            let start = InternalKey::encode(t[0].0.as_ref(), t[0].1, t[0].2);
            let limit = InternalKey::encode(t[1].0.as_ref(), t[1].1, t[1].2);
            let res = InternalKeyComparator::default().find_shortest_separator(&start, &limit);
            assert_eq!(res, InternalKey::encode(t[2].0.as_ref(), t[2].1, t[2].2));
        }
    }

    #[test]
    fn test_intetnal_key_short_successor() {
        let ik = InternalKey::encode("foo".as_ref(), 100, ValueType::Value);
        let res = InternalKeyComparator::default().find_short_successor(&ik);
        assert_eq!(
            res,
            InternalKey::encode("g".as_ref(), MAX_SEQUENCE_NUMBER, ValueType::Value)
        );
    }
}
