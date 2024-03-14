use std::{
    mem::{self, size_of},
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

const BLOCK_SIZE: u64 = 4096;
const POINTER_LENGTH: usize = size_of::<*mut ()>();

pub struct Arena {
    alloc_ptr: *mut u8,
    bytes_remaining: usize,
    blocks: Vec<Vec<u8>>,
    usage: AtomicUsize,
}

const EXTRA_VEC_LEN: usize = mem::size_of::<Vec<u8>>();

impl Arena {
    #[inline]
    pub fn allocate(&mut self, bytes: usize) -> *mut u8 {
        if bytes <= self.bytes_remaining {
            let result = self.alloc_ptr;
            self.alloc_ptr = unsafe { self.alloc_ptr.add(bytes) };
            self.bytes_remaining -= bytes;
            result
        } else {
            self.allocate_fallback(bytes)
        }
    }

    pub fn allocate_aligned(&mut self, bytes: usize) -> *mut u8 {
        let align = POINTER_LENGTH.max(8);

        let current_mod = self.alloc_ptr as usize & (align - 1);
        let slop = if current_mod == 0 {
            0
        } else {
            align - current_mod
        };

        let needed = bytes + slop;
        if needed <= self.bytes_remaining {
            let result = unsafe { self.alloc_ptr.add(slop) };
            self.alloc_ptr = unsafe { self.alloc_ptr.add(needed) };
            self.bytes_remaining -= needed;
            result
        } else {
            self.allocate_fallback(bytes)
        }
    }

    #[inline]
    pub fn allocate_fallback(&mut self, bytes: usize) -> *mut u8 {
        if bytes > BLOCK_SIZE as usize / 4 {
            self.allocate_new_block(bytes)
        } else {
            self.alloc_ptr = self.allocate_new_block(BLOCK_SIZE as usize);
            self.bytes_remaining = BLOCK_SIZE as usize;
            self.allocate(bytes)
        }
    }

    #[inline]
    pub fn allocate_new_block(&mut self, block_bytes: usize) -> *mut u8 {
        let block = (0..block_bytes).map(|_| 0u8).collect::<Vec<u8>>();
        self.blocks.push(block);
        self.bytes_remaining = block_bytes;
        let ptr = self.blocks.last_mut().unwrap().as_mut_ptr();
        self.usage.fetch_add(block_bytes + EXTRA_VEC_LEN, SeqCst);
        ptr
    }

    #[inline]
    pub fn mem_usage(&self) -> usize {
        self.usage.load(SeqCst)
    }
}

impl Default for Arena {
    fn default() -> Self {
        Arena {
            alloc_ptr: std::ptr::null_mut(),
            bytes_remaining: 0,
            blocks: Vec::new(),
            usage: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena() {
        let mut arena = Arena::default();
        let mut v = Vec::new();
        for i in 0..100 {
            v.push(arena.allocate(100));
            unsafe {
                v[i].write(i as u8);
            }
        }
        v.iter().enumerate().take(100).for_each(|(i, p)| unsafe {
            assert_eq!(p.read(), i as u8);
        });
    }
}
