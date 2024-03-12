use std::{cell::RefCell, ptr::null, rc::Rc, sync::Arc};

use crate::util::arena::Arena;

use super::skiplist::SkipList;



pub struct MemTable<K: Ord> {
    arena: Rc<RefCell<Arena>>,
    table: Arc<SkipList<K>>,
}

// pub struct MemTable<'a> {
//     table: Arc<SkipList<InternalKey<'a>>>,
// }

// impl MemTable {
//     pub fn new() -> Self {
//         let arena = RefCell::new(Arena::new());

//         MemTable {
//             table: Arc::new(SkipList::new(
//                 InternalKey::new(null(), 0, ValueType::Value),
//             )),
//         }
//     }

//     pub fn add(&self, seq: u64, vt: ValueType, key: BytesMut, value: BytesMut) {

//     }
// }
