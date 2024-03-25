use std::cmp::Ordering;

pub trait Comparator {
    /// Compare to byte strings, bytewise.
    fn cmp(&self, a: &[u8], b: &[u8]) -> Ordering;
    /// Return the shortest byte string that compares "Greater" to the first argument and "Less" to
    /// the second one.
    fn find_shortest_separator(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
    /// Return the shortest byte string that compares "Greater" to the argument.
    fn find_short_successor(&self, key: &[u8]) -> Vec<u8>;
}

pub struct ByteWiseComparator;

impl Comparator for ByteWiseComparator {
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
