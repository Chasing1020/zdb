use std::cmp::Ordering::{Equal, Less};

use bytes::{BufMut, BytesMut};

// use crate::util::comparator::{ByteWiseComparator, Comparator};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Deletion = 0x0,
    Value = 0x1,
}

const MAX_SEQUENCE_NUMBER: u64 = (0x1u64 << 56) - 1;

/// A UserKey is the actual key supplied by the calling application, without any internal
/// decorations.
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct UserKey<'a>(&'a [u8]);

impl<'a> UserKey<'a> {
    pub fn new(key: &'a [u8]) -> UserKey<'a> {
        UserKey(key)
    }

    // Find the shortest separator string for a and b, i.e., a string x such that a < x < b.
    pub fn find_shortest_separator(start: &UserKey, limit: &UserKey) -> Vec<u8> {
        let min_length = std::cmp::min(start.0.len(), limit.0.len());
        let mut diff_index = 0;
        while diff_index < min_length && start.0[diff_index] == limit.0[diff_index] {
            diff_index += 1;
        }
        let mut tmp = start.0.to_vec();
        if diff_index < min_length {
            let diff_byte = start.0[diff_index];
            if diff_byte < 0xff && diff_byte + 1 < limit.0[diff_index] {
                tmp[diff_index] = diff_byte + 1;
                tmp = tmp[..diff_index + 1].to_vec();
            }
        }
        tmp
    }

    // Find the shortest string x, such that x > key
    pub fn find_short_successor(key: &UserKey) -> Vec<u8> {
        let mut tmp = key.0.to_vec();
        for i in 0..tmp.len() {
            if tmp[i] != u8::max_value() {
                tmp[i] += 1;
                return tmp.into_iter().take(i + 1).collect();
            }
        }
        tmp
    }
}

/// An InternalKey consists of [key, tag], so it's basically a MemtableKey without the initial
/// length specification. This type is used as item type of MemtableIterator, and as the key
/// type of tables.
/// InteralKey = UserKey + SeqNumber(7B) + Type(1B)
#[derive(PartialEq, Eq, PartialOrd, Debug)]
pub struct InternalKey<'a>(&'a [u8]);

impl<'a> InternalKey<'a> {
    pub fn new(mut key: Vec<u8>, seq: u64, vt: ValueType) -> Vec<u8> {
        key.put_u64((seq << 8) | vt as u64);
        key.to_vec()
    }

    pub fn user_key(&self) -> UserKey {
        UserKey(&self.0[..self.0.len() - 8])
    }

    pub fn seq(&self) -> u64 {
        let mut seq = [0u8; 8];
        seq.copy_from_slice(&self.0[self.0.len() - 8..]);
        u64::from_be_bytes(seq) >> 8
    }

    pub fn value_type(&self) -> ValueType {
        match self.0[self.0.len() - 1] {
            0 => ValueType::Deletion,
            1 => ValueType::Value,
            _ => panic!("Invalid value type"),
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn find_shortest_separator(a: &InternalKey, b: &InternalKey) -> Vec<u8> {
        let sep = UserKey::find_shortest_separator(&a.user_key(), &b.user_key());
        if sep.len() < a.user_key().0.len() && a.user_key().cmp(&UserKey(sep.as_ref())) == Less {
            return InternalKey::new(sep, MAX_SEQUENCE_NUMBER, ValueType::Value);
        }
        a.0.to_vec()
    }

    pub fn find_short_successor(key: &InternalKey) -> Vec<u8> {
        let tmp = key.user_key().0;
        let succ = UserKey::find_short_successor(&UserKey(tmp));
        if succ.len() < key.user_key().0.len()
            && key.user_key().cmp(&&UserKey(succ.as_ref())) == Less
        {
            return InternalKey::new(succ, MAX_SEQUENCE_NUMBER, ValueType::Value);
        }
        key.0.to_vec()
    }
}

impl Ord for InternalKey<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.user_key().cmp(&other.user_key()) {
            Equal => other.seq().cmp(&self.seq()),
            ord => ord,
        }
    }
}

/// A MemtableKey consists of the following elements: [keylen, key, tag, (vallen, value)] where
/// keylen is a varint32 encoding the length of key+tag. tag is a fixed 8 bytes segment encoding
/// the entry type and the sequence number. vallen and value are optional components at the end.
/// MemTableKey = KeyLen(Varint32) + InteralKey + ValLen(Varint32) + Value
pub struct MemtableKey<'a>(&'a [u8]);

// pub struct InternalKeyComparator {
//     user_comparator: ByteWiseComparator,
// }

// impl InternalKeyComparator {
//     fn new(user_comparator: ByteWiseComparator) -> InternalKeyComparator {
//         InternalKeyComparator { user_comparator }
//     }

//     fn user_comparator(&self) -> ByteWiseComparator {
//         self.user_comparator
//     }
// }

// impl Comparator<InternalKey<'_>> for InternalKeyComparator {
//     fn compare<'a, 'b>(&self, a: &'a InternalKey, b: &'b InternalKey) -> std::cmp::Ordering {
//         match a[..a.len() - 8].cmp(&b[..b.len() - 8]) {
//             std::cmp::Ordering::Equal => b[b.len() - 8..b.len()].cmp(&a[a.len() - 8..a.len()]),
//             ord => ord,
//         }
//     }

//     fn name(&self) -> &str {
//         "leveldb.InternalKeyComparator"
//     }

// fn find_shortest_separator(&self, start: &mut InternalKey, limit: &InternalKey) {
//     let user_start = &start[..start.len() - 8];
//     let user_limit = &limit[..limit.len() - 8];

//     let mut tmp = user_start.clone();
//     self.user_comparator
//         .find_shortest_separator(&mut tmp, &limit.key);
//     if tmp.len() < user_start.len()
//         && self.compare(
//             start,
//             &InternalKey::new(tmp.clone(), MAX_SEQUENCE_NUMBER, ValueType::Value),
//         ) == std::cmp::Ordering::Less
//     {
//         user_start = tmp;
//         start.seq = MAX_SEQUENCE_NUMBER;
//         start.vt = ValueType::Value;
//     }
// }

// fn find_short_successor(&self, key: &mut InternalKey) {
//     let mut tmp = key.key.clone();
//     self.user_comparator.find_short_successor(&mut tmp);
//     if tmp.len() < key.key.len()
//         && self.compare(
//             key,
//             &InternalKey::new(tmp.clone(), MAX_SEQUENCE_NUMBER, ValueType::Value),
//         ) == std::cmp::Ordering::Less
//     {
//         key.key = tmp;
//         key.seq = MAX_SEQUENCE_NUMBER;
//         key.vt = ValueType::Value;
//     }
// }
// }

// fn find_shortest_separator(start: &InternalKey, limit: &InternalKey) -> Vec<u8> {
//     let user_start = start.user_key();
//     let user_limit = limit.user_key();

//     let mut tmp = find_user_key_shortest_separator(&user_start, &limit.key);
//     if tmp.len() < user_start.len()
//         && self.compare(
//             start,
//             &InternalKey::new(tmp.clone(), MAX_SEQUENCE_NUMBER, ValueType::Value),
//         ) == std::cmp::Ordering::Less
//     {
//         user_start = tmp;
//         start.seq = MAX_SEQUENCE_NUMBER;
//         start.vt = ValueType::Value;
//     }
// }

// fn find_short_successor(key: &mut InternalKey) {
//     let mut tmp = key.key.clone();
//     self.user_comparator.find_short_successor(&mut tmp);
//     if tmp.len() < key.key.len()
//         && self.compare(
//             key,
//             &InternalKey::new(tmp.clone(), MAX_SEQUENCE_NUMBER, ValueType::Value),
//         ) == std::cmp::Ordering::Less
//     {
//         key.key = tmp;
//         key.seq = MAX_SEQUENCE_NUMBER;
//         key.vt = ValueType::Value;
//     }
// }

#[cfg(test)]
mod tests {
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
            let start = InternalKey::new(Vec::from(t[0].0), t[0].1, t[0].2);
            let limit = InternalKey::new(Vec::from(t[1].0), t[1].1, t[1].2);
            let res = InternalKey::find_shortest_separator(
                &InternalKey(&start.as_ref()),
                &InternalKey(&limit.as_ref()),
            );
            assert_eq!(res, InternalKey::new(Vec::from(t[2].0), t[2].1, t[2].2));
        }
    }

    #[test]
    fn test_intetnal_key_short_successor() {
        let ik = InternalKey::new(Vec::from("foo"), 100, ValueType::Value);
        let res = InternalKey::find_short_successor(&InternalKey(ik.as_ref()));
        assert_eq!(
            res,
            InternalKey::new(Vec::from("g"), MAX_SEQUENCE_NUMBER, ValueType::Value)
        );
    }
}
