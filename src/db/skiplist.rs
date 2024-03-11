use crate::util::arena::Arena;
// use crate::util::comparator::Comparator;
use rand::{rngs::ThreadRng, thread_rng, Rng};

use std::{
    cell::RefCell,
    cmp::Ordering::{Less, Equal},
    mem::size_of,
    ptr,
    rc::Rc,
    slice,
    sync::{
        atomic::{AtomicPtr, AtomicUsize},
        Arc,
    },
};

const MAX_HEIGHT: usize = 12;

pub struct Node<Key> {
    pub key: Key,
    pub nexts: *mut AtomicPtr<Node<Key>>,
    pub height: usize,
}

impl<Key> Node<Key> {
    fn new_ptr(key: Key, height: usize, arena: &RefCell<Arena>) -> *mut Node<Key> {
        let size =
            height * std::mem::size_of::<AtomicPtr<Node<Key>>>() + std::mem::size_of::<Node<Key>>();
        let mem = arena.borrow_mut().allocate_aligned(size);
        let buf = unsafe { slice::from_raw_parts_mut(mem, size) };
        let (node_buf, ptr_buf) = buf.split_at_mut(size_of::<Node<Key>>());
        let node_ptr = node_buf.as_mut_ptr() as *mut Node<Key>;
        let nexts_ptr = ptr_buf.as_mut_ptr() as *mut AtomicPtr<Node<Key>>;

        unsafe {
            for i in 0..height {
                *(nexts_ptr.add(i)) = AtomicPtr::default();
            }
            (*node_ptr).key = key;
            (*node_ptr).nexts = nexts_ptr;
            (*node_ptr).height = height;
        }
        node_ptr
    }

    #[inline]
    fn next(&self, n: usize) -> *mut Node<Key> {
        assert!(n < self.height);
        unsafe { (*self.nexts.add(n as usize)).load(std::sync::atomic::Ordering::Acquire) }
    }

    #[inline]
    fn set_next(&mut self, n: usize, x: *mut Node<Key>) {
        assert!(n < self.height);
        unsafe { (*self.nexts.add(n as usize)).store(x, std::sync::atomic::Ordering::Release) }
    }

    #[inline]
    fn next_no_barrier(&self, n: usize) -> *mut Node<Key> {
        assert!(n < self.height);
        unsafe { (*self.nexts.add(n as usize)).load(std::sync::atomic::Ordering::Relaxed) }
    }

    #[inline]
    fn set_next_no_barrier(&mut self, n: usize, x: *mut Node<Key>) {
        assert!(n < self.height);
        unsafe { (*self.nexts.add(n as usize)).store(x, std::sync::atomic::Ordering::Relaxed) }
    }
}

pub struct SkipList<Key: Ord> {
    // pub comparator: Rc<dyn Comparator<Key>>,
    pub arena: RefCell<Arena>,
    max_height: AtomicUsize,
    head: *mut Node<Key>,
    rng: RefCell<ThreadRng>,
}

impl<Key: Ord> SkipList<Key> {
    pub fn new(
        // comparator: Rc<dyn Comparator<Key>>,
        arena: RefCell<Arena>,
        key: Key,
    ) -> SkipList<Key> {
        let head = Node::new_ptr(key, MAX_HEIGHT, &arena);
        SkipList {
            // comparator,
            arena,
            max_height: AtomicUsize::new(1),
            head,
            rng: RefCell::new(thread_rng()),
        }
    }

    pub fn insert(&self, key: Key) {
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

        let x = Node::new_ptr(key, height, &self.arena);
        for i in 0..height {
            unsafe {
                (*x).set_next_no_barrier(i, (*prev[i]).next_no_barrier(i));
                (*prev[i]).set_next(i, x);
            }
        }
    }

    pub fn contains(&self, key: &Key) -> bool {
        let x = self.find_greater_or_equal(key, None);
        !x.is_null() && self.equal(key, unsafe { &(*x).key })
    }

    fn find_greater_or_equal(
        &self,
        key: &Key,
        mut prev: Option<&mut [*mut Node<Key>]>,
    ) -> *mut Node<Key> {
        let mut x = self.head;
        let mut level = self.get_max_height() - 1;

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

    fn find_less_than(&self, key: &Key) -> *mut Node<Key> {
        let mut x = self.head;
        let mut level = self.get_max_height() - 1;
        assert!(x.is_null() || !self.equal(unsafe { &(*x).key }, key));

        loop {
            let next = unsafe { (*x).next(level) };
            if !next.is_null() && unsafe { &(*next).key }.cmp(key) == Less {
                x = next;
            } else {
                if level == 0 {
                    return x;
                } else {
                    level -= 1;
                }
            }
        }
    }

    fn find_last(&self) -> *mut Node<Key> {
        let mut x = self.head;
        let mut level = self.get_max_height() - 1;
        loop {
            let next = unsafe { (*x).next(level) };
            if next.is_null() {
                if level == 0 {
                    return x;
                } else {
                    level -= 1;
                }
            } else {
                x = next;
            }
        }
    }

    #[inline]
    fn get_max_height(&self) -> usize {
        self.max_height.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[inline]
    fn key_is_after_node(&self, key: &Key, n: *mut Node<Key>) -> bool {
        !n.is_null() && unsafe { &(*n).key }.cmp(key) == Less
    }

    #[inline]
    fn random_height(&self) -> usize {
        const BRANCHING: usize = 4;
        let mut height = 1;
        while height < MAX_HEIGHT && self.rng.borrow_mut().gen_range(0..BRANCHING) % BRANCHING == 0
        {
            height += 1;
        }
        debug_assert!(height > 0 && height <= MAX_HEIGHT, "height = {}", height);
        height
    }

    #[inline]
    fn equal(&self, a: &Key, b: &Key) -> bool {
        a.cmp(b) == Equal
    }
}

pub struct SkipListIterator<K: Ord> {
    skip_list: Arc<SkipList<K>>,
    node: *const Node<K>,
}

impl<K: Ord> SkipListIterator<K> {
    pub fn new(skip_list: Arc<SkipList<K>>) -> Self {
        SkipListIterator {
            skip_list,
            node: ptr::null(),
        }
    }

    #[inline]
    pub fn valid(&self) -> bool {
        !self.node.is_null()
    }

    #[inline]
    pub fn key(&self) -> &K {
        assert!(self.valid());
        unsafe { &(*self.node).key }
    }

    #[inline]
    pub fn next(&mut self) {
        assert!(self.valid());
        self.node = unsafe { (*self.node).next(0) };
    }

    #[inline]
    pub fn prev(&mut self) {
        // Instead of using explicit "prev" links, we just search for the
        // last node that falls before key.
        assert!(self.valid());
        let node = self.skip_list.find_less_than(unsafe { &(*self.node).key });
        if node == self.skip_list.head {
            self.node = ptr::null();
        } else {
            self.node = node;
        }
    }

    #[inline]
    pub fn seek(&mut self, key: &K) {
        self.node = self.skip_list.find_greater_or_equal(key, None)
    }

    #[inline]
    pub fn seek_to_first(&mut self) {
        self.node = unsafe { (*self.skip_list.head).next(0) }
    }

    #[inline]
    pub fn seek_to_last(&mut self) {
        let node = self.skip_list.find_last();
        if node == self.skip_list.head {
            self.node = ptr::null();
        } else {
            self.node = node;
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::RngCore;

    use super::*;
    use std::{
        cmp::Ordering,
        collections::BTreeSet,
        hash::{DefaultHasher, Hasher},
        sync::{
            atomic::{
                AtomicBool, AtomicU64,
                Ordering::{Acquire, Release},
            },
            Condvar, Mutex,
        },
        thread,
    };

    struct U64Comparator;

    // impl Comparator<u64> for U64Comparator {
    //     fn compare(&self, left: &u64, right: &u64) -> Ordering {
    //         left.cmp(right)
    //     }

    //     fn name(&self) -> &'static str {
    //         "skiplist test comparator"
    //     }

    //     fn find_shortest_separator(&self, _start: &mut u64, _limit: &u64) {}
    //     fn find_short_successor(&self, _key: &mut u64) {}
    // }

    #[test]
    fn test_skiplist_empty() {
        let skiplist = SkipList::new(RefCell::new(Arena::new()), 0);
        assert!(!skiplist.contains(&1));
        let mut skip_iter = SkipListIterator::new(Arc::new(skiplist));
        assert!(!skip_iter.valid());
        skip_iter.seek_to_first();
        assert!(!skip_iter.valid());
        skip_iter.seek(&100);
        assert!(!skip_iter.valid());
    }

    #[test]
    fn test_skiplist_insert() {
        let skiplist = SkipList::new(RefCell::new(Arena::new()), 0);
        skiplist.insert(1);
        skiplist.insert(4);
        skiplist.insert(7);
        assert!(skiplist.contains(&1));
        assert!(skiplist.contains(&4));
        assert!(skiplist.contains(&7));
        assert!(!skiplist.contains(&100));
    }

    #[test]
    fn test_skiplist_insert_and_lookup() {
        let mut set = BTreeSet::new();
        let list = Arc::new(SkipList::new(RefCell::new(Arena::new()), 0));
        let mut rng = thread_rng();
        const N: u64 = 2000;
        const R: u64 = 5000;
        for _ in 0..N {
            let k = rng.next_u64() % R;
            if set.insert(k) {
                list.insert(k);
            }
        }
        for i in 0..R {
            if list.contains(&(i as u64)) != set.contains(&(i as u64)) {
                panic!("contains({}) is wrong", i);
            }
        }

        {
            let mut iter = SkipListIterator::new(list.clone());
            assert!(!iter.valid());

            iter.seek(&0);
            assert!(iter.valid());
            assert_eq!(set.iter().next().unwrap(), iter.key());

            iter.seek_to_first();
            assert!(iter.valid());
            assert_eq!(set.iter().next().unwrap(), iter.key());

            iter.seek_to_last();
            assert!(iter.valid());
            assert_eq!(set.iter().last().unwrap(), iter.key());
        }

        // Forward iteration test
        for i in 0..R {
            let mut iter = SkipListIterator::new(list.clone());
            iter.seek(&(i as u64));

            let mut model_iter = set.iter().filter(|&&k| k >= i as u64);
            for _ in 0..3 {
                let model_key = model_iter.next();
                if model_key.is_none() {
                    assert!(!iter.valid());
                    break;
                } else {
                    assert!(iter.valid());
                    assert_eq!(*model_key.unwrap(), *iter.key() as u64);
                }
                iter.next();
            }
        }
    }

    const K: u64 = 4;

    struct State {
        generation: Vec<AtomicU64>,
    }

    impl State {
        pub fn new() -> Self {
            State {
                generation: (0..K).map(|_| AtomicU64::new(0)).collect(),
            }
        }

        pub fn get(&self, k: u64) -> u64 {
            self.generation.get(k as usize).unwrap().load(Acquire)
        }

        pub fn set(&self, k: u64, v: u64) {
            self.generation.get(k as usize).unwrap().store(v, Release)
        }
    }

    struct ConcurrentTest {
        current: State,
        list: Arc<SkipList<u64>>,
    }

    unsafe impl Sync for ConcurrentTest {}
    unsafe impl Send for ConcurrentTest {}

    impl ConcurrentTest {
        pub fn new() -> Self {
            let arena = RefCell::new(Arena::new());
            // let comparator = Rc::new(U64Comparator {});
            let list = Arc::new(SkipList::new(arena, 0));
            let current = State::new();
            ConcurrentTest { current, list }
        }

        pub fn key(key: u64) -> u64 {
            key >> 40
        }

        pub fn gen(key: u64) -> u64 {
            ((key) >> 8) & 0xffffffff
        }

        pub fn hash(key: u64) -> u64 {
            key & 0xff
        }

        pub fn hash_numbers(k: u64, g: u64) -> u64 {
            let mut hash = DefaultHasher::new();
            hash.write_u64(k);
            hash.write_u64(g);
            hash.finish()
        }

        pub fn make_key(k: u64, g: u64) -> u64 {
            assert!(k <= K);
            assert!(g <= 0xffffffff);
            (k << 40) | (g << 8) | (ConcurrentTest::hash_numbers(k, g) & 0xff)
        }

        pub fn is_valid_key(k: u64) -> bool {
            let key = ConcurrentTest::key(k);
            let gen = ConcurrentTest::gen(k);
            ConcurrentTest::hash(k) == ConcurrentTest::hash_numbers(key, gen) & 0xff
        }

        pub fn random_target(rnd: &mut ThreadRng) -> u64 {
            match rnd.next_u64() % 10 {
                0 => ConcurrentTest::make_key(0, 0),
                1 => ConcurrentTest::make_key(K, 0),
                _ => ConcurrentTest::make_key(rnd.next_u64() % K, 0),
            }
        }

        // REQUIRES: External synchronization
        pub fn write_step(&self, rnd: &mut ThreadRng) {
            let k = rnd.next_u64() % K;
            let g = self.current.get(k) + 1;
            let key = ConcurrentTest::make_key(k, g);
            self.list.insert(key);
            self.current.set(k, g);
        }

        pub fn read_step(&self, rnd: &mut ThreadRng) {
            let initial_state = State::new();
            for i in 0..K {
                initial_state.set(i, self.current.get(i));
            }

            let mut pos = ConcurrentTest::random_target(rnd);
            let mut iter = SkipListIterator::new(self.list.clone());
            iter.seek(&pos);
            loop {
                let current = if iter.valid() {
                    let key = *iter.key();
                    assert!(ConcurrentTest::is_valid_key(key));
                    key
                } else {
                    ConcurrentTest::make_key(K, 0)
                };
                assert!(pos <= current);

                // Verify that everything in [pos,current) was not present in
                // initial_state.
                while pos < current {
                    let k = ConcurrentTest::key(pos);
                    let g = ConcurrentTest::gen(pos);
                    assert!(k < K);
                    assert!(g == 0 || g > initial_state.get(k));

                    pos = if k < ConcurrentTest::key(current) {
                        ConcurrentTest::make_key(k + 1, 0)
                    } else {
                        ConcurrentTest::make_key(k, g + 1)
                    }
                }

                if !iter.valid() {
                    break;
                }

                if rnd.next_u64() % 2 > 0 {
                    iter.next();
                    let k = ConcurrentTest::key(pos);
                    let g = ConcurrentTest::gen(pos);
                    pos = ConcurrentTest::make_key(k, g + 1);
                } else {
                    let new_target = ConcurrentTest::random_target(rnd);
                    if new_target > pos {
                        pos = new_target;
                        iter.seek(&new_target);
                    }
                }
            }
        }
    }

    #[test]
    fn test_concurrent_without_threads() {
        let test = ConcurrentTest::new();
        let mut rnd = thread_rng();
        for _ in 0..1000 {
            test.read_step(&mut rnd);
            test.write_step(&mut rnd)
        }
    }

    #[derive(PartialEq, Eq, Copy, Clone)]
    enum ReaderState {
        Starting,
        Running,
        Done,
    }

    struct TestState {
        pub test: ConcurrentTest,
        pub quit_flag: AtomicBool,
        state: (Mutex<ReaderState>, Condvar),
    }

    impl TestState {
        pub fn new() -> Self {
            let test = ConcurrentTest::new();
            let quit_flag = AtomicBool::new(false);
            let state = (Mutex::new(ReaderState::Starting), Condvar::new());

            TestState {
                test,
                quit_flag,
                state,
            }
        }

        pub fn wait(&self, s: ReaderState) {
            let (mu, cond) = &self.state;
            let mut guard = mu.lock().unwrap();
            while *guard != s {
                guard = cond.wait(guard).unwrap();
            }
        }

        pub fn change(&self, s: ReaderState) {
            let (mu, cond) = &self.state;
            let mut guard = mu.lock().unwrap();
            *guard = s;
            cond.notify_one();
        }
    }

    #[test]
    fn test_run_concurrent() {
        let n = 1000;
        let size = 1000;
        let mut handles = Vec::new();
        for _ in 0..n {
            let state_read = Arc::new(TestState::new());
            let state_write = Arc::clone(&state_read);
            let handle = thread::spawn(move || {
                let mut rnd = thread_rng();

                state_read.change(ReaderState::Running);
                while !state_read.quit_flag.load(Acquire) {
                    state_read.test.read_step(&mut rnd);
                }
                state_read.change(ReaderState::Done)
            });
            handles.push(handle);

            let mut rnd = thread_rng();
            state_write.wait(ReaderState::Running);
            for _ in 0..size {
                state_write.test.write_step(&mut rnd);
            }
            state_write.quit_flag.store(true, Release);
            state_write.wait(ReaderState::Done);
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
