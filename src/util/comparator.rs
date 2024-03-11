use std::cmp::Ordering;

pub trait Comparator<T> {
    fn compare<'a>(&self, a: &'a T, b: &'a T) -> Ordering;
    // fn find_shortest_separator(&self, start: &T, limit: &T) -> Vec<u8>;
    // fn find_short_successor(&self, key: &T) -> Vec<u8>;
}

#[derive(Clone, Copy)]
pub struct ByteWiseComparator;

impl Comparator<&[u8]> for ByteWiseComparator {
    fn compare<'a, 'b>(&self, a: &'a &[u8], b: &'b &[u8]) -> Ordering {
        a.cmp(b)
    }
    // Find the shortest separator string for a and b, i.e., a string x such that a < x < b.
    // fn find_shortest_separator(&self, mut start: &mut &[u8], limit: &&[u8]) {
    //     let min_length = std::cmp::min(start.len(), limit.len());
    //     let mut diff_index = 0;
    //     while diff_index < min_length && start[diff_index] == limit[diff_index] {
    //         diff_index += 1;
    //     }
    //     if diff_index < min_length {
    //         let diff_byte = start[diff_index];
    //         if diff_byte < 0xff && diff_byte + 1 < limit[diff_index] {
    //             start[diff_index] = diff_byte + 1;
    //             start = &mut &start[..diff_index + 1];
    //         }
    //     }
    // }

    // // Find the shortest string x, such that x > key
    // fn find_short_successor(&self, mut key: &mut &[u8]) {
    //     for i in 0..key.len() {
    //         if key[i] != u8::max_value() {
    //             key[i] += 1;
    //             key = &mut &key[..i + 1];
    //             return;
    //         }
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare() {
        assert_eq!(
            ByteWiseComparator.compare(&"abcd".as_bytes(), &"abce".as_bytes()),
            Ordering::Less
        );
        assert_eq!(
            ByteWiseComparator.compare(&"abcd".as_bytes(), &"abcd".as_bytes()),
            Ordering::Equal
        );
        assert_eq!(
            ByteWiseComparator.compare(&"abcd".as_bytes(), &"abcf".as_bytes()),
            Ordering::Greater
        );
    }

    // #[test]
    // fn test_find_shortest_separator() {
    //     let mut start = "abcd".as_bytes();
    //     let limit = "abcf".as_bytes();
    //     ByteWiseComparator.find_shortest_separator(&mut start, &limit);
    //     assert_eq!(start, BytesMut::from("abce"));

    //     let mut start = "foo".as_bytes();
    //     let limit = "hello".as_bytes();
    //     ByteWiseComparator.find_shortest_separator(&mut start, &limit);
    //     assert_eq!(start, BytesMut::from("g"));
    // }

    // #[test]
    // fn test_find_short_successor() {
    //     let mut key = "abcd".as_bytes();
    //     ByteWiseComparator.find_short_successor(&mut key);
    //     assert_eq!(key, BytesMut::from("b"));

    //     let mut key = "zzzz".as_bytes();
    //     ByteWiseComparator.find_short_successor(&mut key);
    //     assert_eq!(key, BytesMut::from("{"));
    // }
}
