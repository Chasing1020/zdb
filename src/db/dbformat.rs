use num_enum::FromPrimitive;
use std::{
    borrow::Borrow,
    cmp::Ordering::{Equal, Less},
    convert::AsRef,
    fmt::{Debug, Display, Formatter},
    mem,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use bytes::{buf::Buf, BufMut};

use crate::util::comparator::{self, Comparator};

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
                tmp.truncate(i+1);
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
pub struct InternalKey<T: AsRef<[u8]>> {
    pub user_key: UserKey<T>,
    pub seq: u64,
    pub vt: ValueType,
}

impl<T: AsRef<[u8]>> Display for InternalKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InternalKey: {:?} {:?} {:?}",
            self.user_key, self.seq, self.vt
        )
    }
}

impl<T: AsRef<[u8]>> Debug for InternalKey<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InternalKey: {:?} {:?} {:?}",
            self.user_key, self.seq, self.vt
        )
    }
}

impl<T: AsRef<[u8]>> InternalKey<T> {
    pub fn new(user_key: T, seq: u64, vt: ValueType) -> Self {
        InternalKey {
            user_key: UserKey(user_key),
            seq,
            vt,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.user_key.len() + INTERAL_SUFFIX_LEN
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.user_key.as_ref().is_empty()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut ik = self.user_key.as_ref().to_vec();
        ik.put_u64((self.seq << 8) | self.vt as u64);
        ik
    }

    pub fn encode_into(&self, buf: &mut impl BufMut) {
        buf.put_slice(self.user_key.as_ref());
        buf.put_u64((self.seq << 8) | self.vt as u64);
    }
}

impl<'a> InternalKey<&'a [u8]> {
    pub fn decode(slice: &'a [u8]) -> Self {
        let s = slice.as_ref();
        let suffix = (&s[s.len() - INTERAL_SUFFIX_LEN..]).get_u64();

        Self {
            user_key: UserKey(&s[..s.len() - INTERAL_SUFFIX_LEN]),
            seq: suffix >> 8,
            vt: ValueType::from((suffix & 0xff) as u8),
        }
    }
}

impl<T: AsRef<[u8]> + Ord + Eq> Ord for InternalKey<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.user_key
            .cmp(&other.user_key)
            .then(other.seq.cmp(&self.seq))
    }
}

impl<T: AsRef<[u8]> + Ord + Eq> PartialOrd for InternalKey<T> {
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
        let a = InternalKey::decode(a);
        let b = InternalKey::decode(b);

        self.comparator
            .cmp(&a.user_key, &b.user_key)
            .then(b.seq.cmp(&a.seq))
    }

    fn find_shortest_separator(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
        let ik_from = InternalKey::decode(from);
        let ik_to = InternalKey::decode(to);

        let sep = self
            .comparator
            .find_shortest_separator(&ik_from.user_key, &ik_to.user_key);
        if sep.len() < ik_from.user_key.len()
            && self.comparator.cmp(from, &sep) == Less
        {
            InternalKey::new(&sep, MAX_SEQUENCE_NUMBER, ValueType::Value).encode()
        } else {
            InternalKey::new(ik_from.user_key, ik_from.seq, ik_from.vt).encode()
        }
    }

    fn find_short_successor(&self, key: &[u8]) -> Vec<u8> {
        let ik = InternalKey::decode(key);
        let mut tmp = self.comparator.find_short_successor(&ik.user_key);
        if tmp.len() < ik.user_key.len() {
            tmp.put_u64(MAX_SEQUENCE_NUMBER << 8 | ValueType::Value as u64);
            return tmp;
        }
        ik.encode()
    }
}



// /// A MemtableKey consists of the following elements: [keylen, key, tag, (vallen, value)] where
// /// keylen is a varint32 encoding the length of key+tag. tag is a fixed 8 bytes segment encoding
// /// the entry type and the sequence number. vallen and value are optional components at the end.
// /// MemTableKey = KeyLen(Varint32) + InteralKey + ValLen(Varint32) + Value
// pub struct MemtableKey<'a>(&'a [u8]);


// A helper class useful for DBImpl::Get()
pub struct LookupKey {
    // We construct a u8 buf of the form:
    //    klength  varint32
    //    userkey  char[klength]          <-- user_key_start
    //    tag      uint64
    //                                    <-- end
    // The array is a suitable MemTable key.
    // The suffix starting with "userkey" can be used as an InternalKey.
    user_key_start: usize,
    end: usize,
    // if long_key_buf is none, that means the key was stored in short_key_buf
    long_key_buf: Option<Vec<u8>>,
    short_key_buf: [u8; 200],
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
            let start = InternalKey::new(Vec::from(t[0].0), t[0].1, t[0].2).encode();
            let limit = InternalKey::new(Vec::from(t[1].0), t[1].1, t[1].2).encode();
            let res = InternalKeyComparator::default().find_shortest_separator(&start, &limit);
            assert_eq!(
                res,
                InternalKey::new(Vec::from(t[2].0), t[2].1, t[2].2).encode()
            );
        }
    }

    #[test]
    fn test_intetnal_key_short_successor() {
        let ik = InternalKey::new(Vec::from("foo"), 100, ValueType::Value).encode();
        let res = InternalKeyComparator::default().find_short_successor(&ik);
        assert_eq!(
            res,
            InternalKey::new(Vec::from("g"), MAX_SEQUENCE_NUMBER, ValueType::Value).encode()
        );
    }
}
