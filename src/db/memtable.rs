use std::{cell::RefCell, io::Write, sync::Arc};

use super::{
    dbformat::{LookupKey, MemTableKey, UserKey, ValueType},
    error::StorageError,
    skiplist::{SkipList, SkipListIterator},
};
use crate::{db::dbformat::InternalKey, util::arena::Arena};
use integer_encoding::{VarInt, VarIntWriter};

// /// MemTableKey = KeyLen(Varint32) + InteralKey + ValLen(Varint32) + Value
// pub struct MemtableKey<'a>(&'a [u8]);
// TODO: implement MemtableKey

pub struct MemTable<K: AsRef<[u8]> + From<Vec<u8>> + Ord> {
    table: Arc<SkipList<MemTableKey<K>>>,
}

impl<K: AsRef<[u8]> + From<Vec<u8>> + Ord> MemTable<K> {
    pub fn add(&mut self, seq: u64, vt: ValueType, key: UserKey<&[u8]>, value: &[u8]) {
        self.table
            .insert(Self::build_memtable_key(seq, vt, key, value).into());
    }

    pub fn get(&self, lk: &LookupKey) -> Result<Option<Vec<u8>>, StorageError> {
        let mut iter = SkipListIterator::new(self.table.clone());
        iter.seek(&MemTableKey::<K>::from(lk.key.clone()));
        if iter.valid() {
            let foundkey = iter.key().as_ref();
            // memtable_key = KeyLen(Varint32) + InteralKey + ValLen(Varint32) + Value
            let (key_len, key_off, tag, val_len, val_off) = Self::parse_memtable_key(foundkey);

            if lk.user_key().as_ref() == &foundkey[key_off..key_off + key_len] {
                if tag & 0xff == ValueType::Value as u64 {
                    return Ok(Some(foundkey[val_off..val_off + val_len].to_vec()));
                } else {
                    return Ok(None);
                }
            }
        }
        Err(StorageError::NotFound(format!("{:?} not found", lk)))
    }

    pub fn approximate_mem_usage(&self) -> usize {
        self.table.approximate_mem_usage()
    }

    fn parse_memtable_key(key: &[u8]) -> (usize, usize, u64, usize, usize) {
        let (keylen, mut i) = usize::decode_var(key).unwrap();
        let keyoff = i;
        i += keylen - 8;

        if key.len() > i {
            let tag = u64::from_be_bytes(key[i..i + 8].try_into().unwrap());
            i += 8;
            let (val_len, j) = VarInt::decode_var(&key[i..]).unwrap();
            i += j;
            let valoff = i;
            (keylen - 8, keyoff, tag, val_len, valoff)
        } else {
            (keylen - 8, keyoff, 0, 0, 0)
        }
    }

    fn build_memtable_key(seq: u64, vt: ValueType, key: UserKey<&[u8]>, value: &[u8]) -> Vec<u8> {
        let key_len = key.len();
        let val_len = value.len();
        let internal_key_len = key_len + 8;
        let memtable_key = (internal_key_len as u64).required_space()
            + internal_key_len
            + (val_len as u64).required_space()
            + val_len;

        let mut buf = (0..memtable_key).map(|_| 0).collect::<Vec<u8>>();
        let mut writer = buf.as_mut_slice();

        // /// MemTableKey = KeyLen(Varint32) + InteralKey + ValLen(Varint32) + Value
        writer
            .write_varint(internal_key_len)
            .expect("write internal_key_len failed");

        let ik = InternalKey::encode(&key, seq, vt);
        writer
            .write_all(ik.as_ref())
            .expect("write internal_key failed");

        writer.write_varint(val_len).expect("write to slice failed");
        writer.write_all(value).expect("write to slice failed");
        buf
    }
}

impl<K: AsRef<[u8]> + From<Vec<u8>> + Ord> Default for MemTable<K> {
    fn default() -> Self {
        let arena = RefCell::new(Arena::default());
        MemTable {
            table: Arc::new(SkipList::new(arena, vec![].into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memtable() {
        let mut memtable = MemTable::<Vec<u8>>::default();
        let key = b"key".to_vec();
        let value = b"value".to_vec();
        let seq = 1;
        let vt = ValueType::Value;
        let user_key = UserKey(key.as_slice());
        memtable.add(seq, vt, user_key, &value);
        let lk = LookupKey::new(UserKey(&key), seq, vt);
        let result = memtable.get(&lk);
        assert_eq!(result.unwrap().unwrap(), value);
    }
}
