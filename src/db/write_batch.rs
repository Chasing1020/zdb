use super::dbformat::ValueType;
use bytes::{BufMut, BytesMut};
use std::fmt::{Debug, Formatter};

const HEADER: usize = 12;

pub struct WriteBatch {
    rep: BytesMut,
}

impl Default for WriteBatch {
    fn default() -> Self {
        Self {
            rep: BytesMut::from(&[0u8; HEADER][..]),
        }
    }
}

impl WriteBatch {
    pub fn new() -> WriteBatch {
        WriteBatch::default()
    }

    pub fn put(&mut self, key: &[u8], value: &[u8]) {
        self.set_count(self.count() + 1);
        self.rep.put_u8(ValueType::Value as u8);
        self.rep.put_u8(key.len() as u8);
        self.rep.put_slice(key);
        self.rep.put_u8(value.len() as u8);
        self.rep.put_slice(value);
    }

    pub fn delete(&mut self, key: &[u8]) {
        self.set_count(self.count() + 1);
        self.rep.put_u8(ValueType::Deletion as u8);
        self.rep.put_u8(key.len() as u8);
        self.rep.put_slice(key);
    }

    fn set_sequence(&mut self, seq: u64) {
        for i in 0..8 {
            self.rep[i] = (seq >> (i * 8)) as u8;
        }
    }

    fn sequence(&self) -> u64 {
        let mut result = 0;
        for i in 0..8 {
            result |= (self.rep[i] as u64) << (i * 8);
        }
        result
    }

    fn set_count(&mut self, n: u32) {
        for i in 0..HEADER - 8 {
            self.rep[i + 8] = (n >> (i * 8)) as u8;
        }
    }

    fn count(&self) -> u32 {
        let mut result = 0;
        for i in 0..HEADER - 8 {
            result |= (self.rep[i + 8] as u32) << (i * 8);
        }
        result
    }

    pub fn data(&self) -> &[u8] {
        &self.rep
    }
}

impl Debug for WriteBatch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteBatch")
            .field("rep", &self.rep)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writebatch() {
        let mut batch = WriteBatch::new();
        batch.set_sequence(100);
        batch.put(b"foo", b"bar");
        batch.delete(b"baz");
        batch.put(b"baz", b"foo");
        assert!(batch.count() == 3);
        assert!(batch.sequence() == 100);
        dbg!(&batch);
    }
}
