use bytes::{BufMut, BytesMut};

use crate::util::comparator::{ByteWiseComparator, Comparator};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Deletion = 0x0,
    Value = 0x1,
}

const MAX_SEQUENCE_NUMBER: u64 = (0x1u64 << 56) - 1;

#[derive(Debug, Eq, PartialEq)]
pub struct InternalKey {
    pub key: BytesMut,
    pub seq: u64,
    pub vt: ValueType,
}

impl InternalKey {
    pub fn new(key: BytesMut, seq: u64, vt: ValueType) -> InternalKey {
        InternalKey { key, seq, vt }
    }

    pub fn encode(&self) -> BytesMut {
        let mut result = BytesMut::new();
        assert!(self.seq <= MAX_SEQUENCE_NUMBER);
        result.put_slice(&self.key);
        result.put_u64_le(self.seq << 8 | self.vt as u64);
        result
    }
}

struct InternalKeyComparator {
    user_comparator: ByteWiseComparator,
}

impl InternalKeyComparator {
    fn new(user_comparator: ByteWiseComparator) -> InternalKeyComparator {
        InternalKeyComparator { user_comparator }
    }

    fn user_comparator(&self) -> ByteWiseComparator {
        self.user_comparator
    }
}

impl Comparator<InternalKey> for InternalKeyComparator {
    fn compare<'a>(&self, a: &'a InternalKey, b: &'a InternalKey) -> std::cmp::Ordering {
        match a.key.cmp(&b.key) {
            std::cmp::Ordering::Equal => b.seq.cmp(&a.seq),
            ord => ord,
        }
    }

    fn name(&self) -> &str {
        "leveldb.InternalKeyComparator"
    }

    fn find_shortest_separator(&self, start: &mut InternalKey, limit: &InternalKey) {
        let mut tmp = start.key.clone();
        self.user_comparator
            .find_shortest_separator(&mut tmp, &limit.key);
        if tmp.len() < start.key.len()
            && self.compare(
                start,
                &InternalKey::new(tmp.clone(), MAX_SEQUENCE_NUMBER, ValueType::Value),
            ) == std::cmp::Ordering::Less
        {
            start.key = tmp;
            start.seq = MAX_SEQUENCE_NUMBER;
            start.vt = ValueType::Value;
        }
    }

    fn find_short_successor(&self, key: &mut InternalKey) {
        let mut tmp = key.key.clone();
        self.user_comparator.find_short_successor(&mut tmp);
        if tmp.len() < key.key.len()
            && self.compare(
                key,
                &InternalKey::new(tmp.clone(), MAX_SEQUENCE_NUMBER, ValueType::Value),
            ) == std::cmp::Ordering::Less
        {
            key.key = tmp;
            key.seq = MAX_SEQUENCE_NUMBER;
            key.vt = ValueType::Value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_key_short_separator() {
        let comparator = InternalKeyComparator::new(ByteWiseComparator);
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
            let mut start = InternalKey::new(BytesMut::from(t[0].0), t[0].1, t[0].2);
            let limit = InternalKey::new(BytesMut::from(t[1].0), t[1].1, t[1].2);
            comparator.find_shortest_separator(&mut start, &limit);
            assert_eq!(
                start,
                InternalKey::new(BytesMut::from(t[2].0), t[2].1, t[2].2)
            );
        }
    }

    #[test]
    fn test_intetnal_key_short_successor() {
        let comparator = InternalKeyComparator::new(ByteWiseComparator);
        let mut ik = InternalKey::new(BytesMut::from("foo"), 100, ValueType::Value);
        comparator.find_short_successor(&mut ik);
        assert_eq!(
            ik,
            InternalKey::new(BytesMut::from("g"), MAX_SEQUENCE_NUMBER, ValueType::Value)
        );
    }
}
