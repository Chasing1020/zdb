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
