use std::cmp::Ordering;

use bytes::BytesMut;

pub trait Comparator<T> {
    fn compare<'a>(&self, a: &'a T, b: &'a T) -> Ordering;
    fn name(&self) -> &str;
    fn find_shortest_separator(&self, start: &mut T, limit: &T);
    fn find_short_successor(&self, key: &mut T);
}

#[derive(Clone, Copy)]
pub struct ByteWiseComparator;

impl Comparator<BytesMut> for ByteWiseComparator {
    fn compare<'a>(&self, a: &'a BytesMut, b: &'a BytesMut) -> Ordering {
        a.cmp(b)
    }

    fn name(&self) -> &str {
        "leveldb.BytewiseComparator"
    }

    // Find the shortest separator string for a and b, i.e., a string x such that a < x < b.
    fn find_shortest_separator(&self, start: &mut BytesMut, limit: &BytesMut) {
        let min_length = std::cmp::min(start.len(), limit.len());
        let mut diff_index = 0;
        while diff_index < min_length && start[diff_index] == limit[diff_index] {
            diff_index += 1;
        }
        if diff_index < min_length {
            let diff_byte = start[diff_index];
            if diff_byte < 0xff && diff_byte + 1 < limit[diff_index] {
                start[diff_index] = diff_byte + 1;
                start.truncate(diff_index + 1);
            }
        }
    }

    // Find the shortest string x, such that x > key
    fn find_short_successor(&self, key: &mut BytesMut) {
        for i in 0..key.len() {
            if key[i] != u8::max_value() {
                key[i] += 1;
                key.truncate(i + 1);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare() {
        assert_eq!(
            ByteWiseComparator.compare(&BytesMut::from("abcd"), &BytesMut::from("abce")),
            Ordering::Less
        );
        assert_eq!(
            ByteWiseComparator.compare(&BytesMut::from("abcd"), &BytesMut::from("abcd")),
            Ordering::Equal
        );
        assert_eq!(
            ByteWiseComparator.compare(&BytesMut::from("abce"), &BytesMut::from("abcd")),
            Ordering::Greater
        );
    }

    #[test]
    fn test_find_shortest_separator() {
        let mut start = BytesMut::from("abcd");
        let limit = BytesMut::from("abcf");
        ByteWiseComparator.find_shortest_separator(&mut start, &limit);
        assert_eq!(start, BytesMut::from("abce"));

        let mut start = BytesMut::from("foo");
        let limit = BytesMut::from("hello");
        ByteWiseComparator.find_shortest_separator(&mut start, &limit);
        assert_eq!(start, BytesMut::from("g"));
    }

    #[test]
    fn test_find_short_successor() {
        let mut key = BytesMut::from("abcd");
        ByteWiseComparator.find_short_successor(&mut key);
        assert_eq!(key, BytesMut::from("b"));

        let mut key = BytesMut::from("zzzz");
        ByteWiseComparator.find_short_successor(&mut key);
        assert_eq!(key, BytesMut::from("{"));
    }
}
