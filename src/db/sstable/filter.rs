use bytes::BufMut;
use core::f64::consts::LN_2;

pub trait BitSlice {
    fn get_bit(&self, idx: usize) -> bool;
    fn bit_len(&self) -> usize;
}

pub trait BitSliceMut {
    fn set_bit(&mut self, idx: usize, val: bool);
}

impl<T: AsRef<[u8]>> BitSlice for T {
    fn get_bit(&self, idx: usize) -> bool {
        let pos = idx / 8;
        let offset = idx % 8;
        (self.as_ref()[pos] & (1 << offset)) != 0
    }

    fn bit_len(&self) -> usize {
        self.as_ref().len() * 8
    }
}

impl<T: AsMut<[u8]>> BitSliceMut for T {
    fn set_bit(&mut self, idx: usize, val: bool) {
        let pos = idx / 8;
        let offset = idx % 8;
        if val {
            self.as_mut()[pos] |= 1 << offset;
        } else {
            self.as_mut()[pos] &= !(1 << offset);
        }
    }
}

pub struct BloomBuilder {
    key_hashs: Vec<u32>,
    false_positive_rate: f64,
}

impl BloomBuilder {
    pub fn new(false_positive_rate: f64) -> Self {
        Self {
            key_hashs: vec![],
            false_positive_rate,
        }
    }

    pub fn add(&mut self, key: impl AsRef<[u8]>) {
        self.key_hashs.push(farmhash::hash32(key.as_ref()));
    }

    pub fn build(&self) -> Bloom {
        let mut bloom = Bloom::new(self.key_hashs.len(), self.false_positive_rate);
        self.key_hashs.iter().for_each(|key| {
            bloom.set(*key);
        });
        bloom
    }
}

/// Bloom filter using farmhash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bloom {
    /// data of filter in bits.
    bit_vec: Vec<u8>,
    /// number of hash functions.
    k_num: u8,
}

impl Bloom {
    /// Create a new Bloom filter
    /// `item_count`: number of items expected to be inserted
    /// `false_positive_rate`: expected false positive rate
    pub fn new(item_count: usize, false_positive_rate: f64) -> Self {
        let bits_per_key = Self::bits_per_key(item_count, false_positive_rate);
        let bitmap_size = item_count * bits_per_key;
        let k_num = Self::compute_hash_num(bits_per_key);
        let bit_vec = vec![0u8; (bitmap_size + 7) / 8];
        Self { bit_vec, k_num }
    }

    /// Record the presence of a hash.
    pub fn set(&mut self, hash: u32) {
        let mut h = hash;
        let delta = (hash >> 17) | (hash << 15);
        for _ in 0..self.k_num {
            let bit_pos = (h as usize) % self.bit_vec.bit_len();
            self.bit_vec.set_bit(bit_pos, true);
            h = h.wrapping_add(delta);
        }
    }

    /// Check if an item is present in the filter.
    /// There can be false positives, but no false negatives.
    pub fn check(&self, hash: u32) -> bool {
        let mut h = hash;
        let delta = (hash >> 17) | (hash << 15);
        for _ in 0..self.k_num {
            let bit_pos = (h as usize) % self.bit_vec.bit_len();
            if !self.bit_vec.get_bit(bit_pos) {
                return false;
            }
            h = h.wrapping_add(delta);
        }
        true
    }

    /// Chech and set the hash
    /// Return true if the item is already in the filter
    pub fn check_and_set(&mut self, hash: u32) -> bool {
        let mut h = hash;
        let delta = (hash >> 17) | (hash << 15);
        let mut found = true;
        for _ in 0..self.k_num {
            let bit_pos = (h as usize) % self.bit_vec.bit_len();
            if !self.bit_vec.get_bit(bit_pos) {
                found = false;
                self.bit_vec.set_bit(bit_pos, true);
            }
            h = h.wrapping_add(delta);
        }
        found
    }

    /// Clear the filter, set all bits to 0
    pub fn clear(&mut self) {
        self.bit_vec.clear();
    }

    /// Set all bits to 1, which means the filter is full
    pub fn fill(&mut self) {
        self.bit_vec.iter_mut().for_each(|b| *b = 0xff);
    }

    /// Check if the filter is empty
    pub fn is_empty(&self) -> bool {
        !self.bit_vec.iter().any(|b| *b != 0)
    }

    /// Encode the filter and return a byte vector
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = vec![];
        self.encode_into(&mut buf);
        buf
    }

    /// Encode the filter to a byte vector
    pub fn encode_into(&self, buf: &mut impl BufMut) {
        buf.put_slice(&self.bit_vec);
        buf.put_u8(self.k_num);
    }

    /// Decode the filter from a byte vector
    pub fn decode(buf: &[u8]) -> Self {
        let k_num = buf[buf.len() - 1];
        let bit_vec = buf[..buf.len() - 1].to_vec();
        Self { bit_vec, k_num }
    }

    fn bits_per_key(item_count: usize, fp_rate: f64) -> usize {
        let size = -1.0 * (item_count as f64) * fp_rate.ln() / LN_2.powi(2);
        (size / (item_count as f64)).ceil() as usize
    }

    fn compute_hash_num(bits_per_key: usize) -> u8 {
        assert!(bits_per_key > 0);
        let k = (bits_per_key as f64 * LN_2) as u8;
        k.clamp(1, 25)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom() {
        let mut builder = BloomBuilder::new(0.01);
        for i in 0..100 {
            builder.add(i.to_string());
        }
        let bloom = builder.build();
        for i in 0..100 {
            assert!(bloom.check(farmhash::hash32(i.to_string().as_bytes())));
        }
    }

    fn false_positive_rate_case(item_count: usize, test_count: usize, fp_rate: f64) {
        let mut builder = BloomBuilder::new(fp_rate);
        for i in 0..item_count {
            builder.add(i.to_string().as_bytes());
        }
        let bloom = builder.build();
        let mut false_positive = 0f64;
        for i in item_count..item_count + test_count {
            if bloom.check(farmhash::hash32(i.to_string().as_bytes())) {
                false_positive += 1.0;
            }
        }
        let rate = false_positive / test_count as f64;
        assert!(rate < fp_rate * 3.0);
    }

    #[test]
    fn test_bloom_bits_per_key() {
        false_positive_rate_case(1000000, 100000, 0.1);
        false_positive_rate_case(1000000, 100000, 0.01);
        false_positive_rate_case(1000000, 100000, 0.001);
    }

    #[test]
    fn test_bloom_encode_decode() {
        let item_count = 10000;
        let mut builder = BloomBuilder::new(0.01);
        for i in 0..item_count {
            builder.add(i.to_string());
        }
        let bloom = builder.build();

        let encoded = bloom.encode();
        let decoded = Bloom::decode(&encoded);
        assert_eq!(bloom.bit_vec, decoded.bit_vec);
        for i in 0..item_count {
            assert!(decoded.check(farmhash::hash32(i.to_string().as_bytes())));
        }
        assert_eq!(bloom.k_num, decoded.k_num);
    }
}
