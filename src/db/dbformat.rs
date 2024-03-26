// pub const MAX_SEQUENCE_NUMBER: u64 = (0x1u64 << 56) - 1;
// pub const INTERAL_SUFFIX_LEN: usize = mem::size_of::<u64>();

// #[repr(u8)]
// #[derive(Clone, Copy, Debug, PartialEq, Eq, Default, FromPrimitive)]
// pub enum ValueType {
//     Deletion = 0x0,
//     #[default]
//     Value = 0x1,
// }

// pub struct DefaultComparator;

// impl Comparator for DefaultComparator {
//     fn cmp(&self, a: &[u8], b: &[u8]) -> std::cmp::Ordering {
//         a.cmp(b)
//     }

//     // Find the shortest string x, such that a < x < b
//     fn find_shortest_separator(&self, a: &[u8], b: &[u8]) -> Vec<u8> {
//         let min_length = std::cmp::min(a.len(), b.len());
//         let mut diff_index = 0;
//         while diff_index < min_length && a[diff_index] == b[diff_index] {
//             diff_index += 1;
//         }
//         let mut tmp = a.to_vec();
//         if diff_index < min_length {
//             let diff_byte = a[diff_index];
//             if diff_byte < 0xff && diff_byte + 1 < b[diff_index] {
//                 tmp[diff_index] = diff_byte + 1;
//                 tmp = tmp[..diff_index + 1].to_vec();
//             }
//         }
//         tmp
//     }

//     fn find_short_successor(&self, key: &[u8]) -> Vec<u8> {
//         let mut tmp = key.to_vec();
//         for i in 0..tmp.len() {
//             if key[i] != u8::max_value() {
//                 tmp[i] += 1;
//                 tmp.truncate(i + 1);
//                 return tmp;
//             }
//         }
//         tmp
//     }
// }

// pub struct InternalKeyComparator {
//     pub comparator: Rc<Box<dyn Comparator>>,
// }

// impl InternalKeyComparator {
//     pub fn new(comparator: Box<dyn Comparator>) -> Self {
//         InternalKeyComparator {
//             comparator: Rc::new(comparator),
//         }
//     }
// }

// impl Default for InternalKeyComparator {
//     fn default() -> Self {
//         InternalKeyComparator {
//             comparator: Rc::new(Box::new(DefaultComparator)),
//         }
//     }
// }

// impl Comparator for InternalKeyComparator {
//     fn cmp(&self, a: &[u8], b: &[u8]) -> std::cmp::Ordering {
//         let a = InternalKey(a);
//         let b = InternalKey(b);
//         self.comparator
//             .cmp(a.user_key(), b.user_key())
//             .then(b.seq().cmp(&a.seq()))
//     }

//     fn find_shortest_separator(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
//         let ik_from = InternalKey(from);
//         let ik_to = InternalKey(to);

//         let sep = self
//             .comparator
//             .find_shortest_separator(ik_from.user_key(), ik_to.user_key());
//         if sep.len() < ik_from.user_key().len() && self.comparator.cmp(from, &sep) == Less {
//             InternalKey::encode(&sep, MAX_SEQUENCE_NUMBER, ValueType::Value)
//         } else {
//             InternalKey::encode(ik_from.user_key(), ik_from.seq(), ik_from.vt())
//         }
//     }

//     fn find_short_successor(&self, key: &[u8]) -> Vec<u8> {
//         let ik = InternalKey(key);
//         let mut tmp = self.comparator.find_short_successor(ik.user_key());
//         if tmp.len() < ik.user_key().len() {
//             tmp.put_u64(MAX_SEQUENCE_NUMBER << 8 | ValueType::Value as u64);
//             return tmp;
//         }
//         ik.as_ref().to_vec()
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_internal_key_short_separator() {
//         let tests = vec![
//             vec![
//                 ("foo", 100, ValueType::Value),
//                 ("foo", 99, ValueType::Value),
//                 ("foo", 100, ValueType::Value),
//             ],
//             vec![
//                 ("foo", 100, ValueType::Value),
//                 ("foo", 101, ValueType::Value),
//                 ("foo", 100, ValueType::Value),
//             ],
//             vec![
//                 ("foo", 100, ValueType::Value),
//                 ("hello", 200, ValueType::Value),
//                 ("g", MAX_SEQUENCE_NUMBER, ValueType::Value),
//             ],
//             vec![
//                 ("foo", 100, ValueType::Value),
//                 ("foobar", 200, ValueType::Value),
//                 ("foo", 100, ValueType::Value),
//             ],
//         ];

//         for t in tests {
//             let start = InternalKey::encode(t[0].0.as_ref(), t[0].1, t[0].2);
//             let limit = InternalKey::encode(t[1].0.as_ref(), t[1].1, t[1].2);
//             let res = InternalKeyComparator::default().find_shortest_separator(&start, &limit);
//             assert_eq!(res, InternalKey::encode(t[2].0.as_ref(), t[2].1, t[2].2));
//         }
//     }

//     #[test]
//     fn test_intetnal_key_short_successor() {
//         let ik = InternalKey::encode("foo".as_ref(), 100, ValueType::Value);
//         let res = InternalKeyComparator::default().find_short_successor(&ik);
//         assert_eq!(
//             res,
//             InternalKey::encode("g".as_ref(), MAX_SEQUENCE_NUMBER, ValueType::Value)
//         );
//     }
// }
