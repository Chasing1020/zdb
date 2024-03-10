use crate::util::arena::Arena;
use crate::util::comparator::Comparator;
use rand::{thread_rng, Rng};

use std::{
    cell::RefCell,
    mem::size_of,
    rc::Rc,
    slice,
    sync::atomic::{AtomicPtr, AtomicUsize},
};

const MAX_HEIGHT: usize = 12;

pub struct Node<Key> {
    pub key: Key,
    pub nexts: *mut AtomicPtr<Node<Key>>,
    pub height: usize,
}

impl<Key> Node<Key> {
    fn new(key: Key, height: usize, arena: &RefCell<Arena>) -> *mut Node<Key> {
        let size =
            height * std::mem::size_of::<AtomicPtr<Node<Key>>>() + std::mem::size_of::<Node<Key>>();
        let mem = arena.borrow_mut().allocate_aligned(size);
        let buf = unsafe { slice::from_raw_parts_mut(mem, size) };
        let (node_buf, ptr_buf) = buf.split_at_mut(size_of::<Node<Key>>());
        let node_ptr = node_buf.as_mut_ptr() as *mut Node<Key>;

        let mut next_vec = unsafe {
            Vec::from_raw_parts(
                ptr_buf.as_mut_ptr() as *mut AtomicPtr<Node<Key>>,
                height,
                height,
            )
        };
        for i in 0..height {
            next_vec[i] = AtomicPtr::default();
        }

        unsafe {
            (*node_ptr).key = key;
            (*node_ptr).nexts = next_vec.as_mut_ptr();
            (*node_ptr).height = height;
        };
        std::mem::forget(next_vec);

        node_ptr
    }

    #[inline]
    pub fn next(&mut self, n: usize) -> *mut Node<Key> {
        unsafe { (*self.nexts.add(n as usize)).load(std::sync::atomic::Ordering::Acquire) }
    }

    #[inline]
    pub fn set_next(&mut self, n: usize, x: *mut Node<Key>) {
        unsafe { (*self.nexts.add(n as usize)).store(x, std::sync::atomic::Ordering::Release) }
    }

    #[inline]
    pub fn next_no_barrier(&mut self, n: usize) -> *mut Node<Key> {
        unsafe { (*self.nexts.add(n as usize)).load(std::sync::atomic::Ordering::Relaxed) }
    }

    #[inline]
    pub fn set_next_no_barrier(&mut self, n: usize, x: *mut Node<Key>) {
        unsafe { (*self.nexts.add(n as usize)).store(x, std::sync::atomic::Ordering::Relaxed) }
    }
}

pub struct SkipList<Key> {
    pub comparator: Rc<dyn Comparator<Key>>,
    pub arena: RefCell<Arena>,
    pub max_height: AtomicUsize,
    pub head: *mut Node<Key>,
}

impl<Key> SkipList<Key> {
    pub fn new(comparator: Rc<dyn Comparator<Key>>, key: Key) -> SkipList<Key> {
        let arena = RefCell::new(Arena::new());
        let head = Node::new(key, MAX_HEIGHT, &arena);
        SkipList {
            comparator: comparator,
            arena,
            max_height: AtomicUsize::new(1),
            head,
        }
    }

    pub fn insert(&mut self, key: Key) {
        let mut prev = [std::ptr::null_mut(); MAX_HEIGHT];
        let x = self.find_greater_or_equal(&key, Some(&mut prev));
        // don't allow duplicate insertion
        assert!(x.is_null() || !self.equal(unsafe { &(*x).key }, &key));

        let height = self.random_height();
        if height > self.get_max_height() {
            for i in self.get_max_height()..height {
                prev[i] = self.head;
            }
            self.max_height
                .store(height, std::sync::atomic::Ordering::Release);
        }

        let x = Node::new(key, height, &self.arena);
        for i in 0..height {
            unsafe {
                (*x).set_next_no_barrier(i, (*prev[i]).next_no_barrier(i));
                (*prev[i]).set_next(i, x);
            }
        }
    }

    fn contains(&self, key: &Key) -> bool {
        let x = self.find_greater_or_equal(key, None);
        if x.is_null() {
            false
        } else {
            self.equal(key, unsafe { &(*x).key })
        }
    }

    fn find_greater_or_equal(
        &self,
        key: &Key,
        mut prev: Option<&mut [*mut Node<Key>]>,
    ) -> *mut Node<Key> {
        let mut x = self.head;
        let mut level = self.max_height.load(std::sync::atomic::Ordering::Acquire) - 1;
        debug_assert!(x.is_null() || !self.equal(unsafe { &(*x).key }, key));

        loop {
            let next = unsafe { (*x).next(level) };
            if self.key_is_after_node(key, next) {
                x = next;
            } else {
                if let Some(ref mut prev_rout) = prev {
                    assert_eq!(prev_rout.len(), MAX_HEIGHT);
                    prev_rout[level] = x;
                }
                if level == 0 {
                    return next;
                }
                level -= 1;
            }
        }
    }

    #[inline]
    fn get_max_height(&self) -> usize {
        self.max_height.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[inline]
    fn key_is_after_node(&self, key: &Key, n: *mut Node<Key>) -> bool {
        !n.is_null()
            && self.comparator.compare(key, unsafe { &(*n).key }) == std::cmp::Ordering::Less
    }

    #[inline]
    fn random_height(&self) -> usize {
        const BRANCHING: usize = 4;
        let mut height = 1;
        let mut rng = thread_rng();
        while height < MAX_HEIGHT && rng.gen_range(1..=BRANCHING) % BRANCHING == 0 {
            height += 1;
        }
        debug_assert!(height > 0 && height <= MAX_HEIGHT, "height = {}", height);
        height
    }

    #[inline]
    fn equal(&self, a: &Key, b: &Key) -> bool {
        self.comparator.compare(a, b) == std::cmp::Ordering::Equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    struct U64Comparator;

    impl Comparator<u64> for U64Comparator {
        fn compare(&self, left: &u64, right: &u64) -> Ordering {
            left.cmp(right)
        }

        fn name(&self) -> &'static str {
            "skiplist test comparator"
        }

        fn find_shortest_separator(&self, _start: &mut u64, _limit: &u64) {}
        fn find_short_successor(&self, _key: &mut u64) {}
    }

    #[test]
    fn test_skiplist() {
        let mut skiplist = SkipList::new(Rc::new(U64Comparator), 0);
        skiplist.insert(1);
        skiplist.insert(4);
        skiplist.insert(7);
        assert!(skiplist.contains(&1));
        assert!(skiplist.contains(&4));
        assert!(skiplist.contains(&7));
        assert!(!skiplist.contains(&100));
    }
}
