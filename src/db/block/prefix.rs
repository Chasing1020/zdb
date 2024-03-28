use std::ops::Range;

use bytes::BytesMut;
use integer_encoding::*;

// EntryPrefix represents the prefix of an entry in the block.
// Each len is the length of the varint encoded size
// The format is:
// ｜ shared_size ｜ non_shared_size  | value_size ｜  non_shared_key   |    value    ｜
//   \shared_len/ + \non_shared_len/ + \value_len/ + \non_shared_size/ + \value_size/ = entry_len
pub struct EntryPrefix {
    // only used for get value range
    pub(crate) offset: usize,

    pub(crate) shared_len: usize,
    pub(crate) shared_size: usize,
    pub(crate) non_shared_len: usize,
    pub(crate) non_shared_size: usize,
    pub(crate) value_len: usize,
    pub(crate) value_size: usize,
}

impl EntryPrefix {
    pub fn decode(buf: &[u8], offset: usize) -> Self {
        let (shared_size, shared_len) = usize::decode_var(buf).unwrap();
        let (non_shared_size, non_shared_len) = usize::decode_var(&buf[shared_len..]).unwrap();
        let (value_size, value_len) =
            usize::decode_var(&buf[shared_len + non_shared_len..]).unwrap();
        Self {
            offset,
            shared_len,
            shared_size,
            non_shared_len,
            non_shared_size,
            value_len,
            value_size,
        }
    }

    #[inline]
    pub fn entry_len(&self) -> usize {
        self.shared_len
            + self.non_shared_len
            + self.value_len
            + self.non_shared_size
            + self.value_size
    }

    #[inline]
    pub fn unshared_key_range(&self) -> Range<usize> {
        self.key_start()..self.key_end()
    }

    #[inline]
    pub fn value_range(&self) -> Range<usize> {
        self.value_start()..self.value_end()
    }

    #[inline]
    fn key_start(&self) -> usize {
        self.offset + self.shared_len + self.non_shared_len + self.value_len
    }

    #[inline]
    fn key_end(&self) -> usize {
        self.key_start() + self.non_shared_size
    }

    #[inline]
    fn value_start(&self) -> usize {
        self.key_end()
    }

    #[inline]
    fn value_end(&self) -> usize {
        self.value_start() + self.value_size
    }

    // TODO: remove this function
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.extend(usize::encode_var_vec(self.shared_size));
        buf.extend(usize::encode_var_vec(self.non_shared_size));
        buf.extend(usize::encode_var_vec(self.value_size));
    }
}
