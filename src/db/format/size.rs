// Copyright 2024 RisingWave Labs. Licensed under Apache-2.0.

use bytes::Bytes;

/// The trait for estimating the actual memory usage of a struct.
///
/// Used for cache eviction now.
pub trait EstimateSize {
    /// The estimated heap size of the current struct in bytes.
    fn estimated_heap_size(&self) -> usize;

    /// The estimated total size of the current struct in bytes, including the `estimated_heap_size`
    /// and the size of `Self`.
    #[inline]
    fn estimated_size(&self) -> usize
    where
        Self: Sized,
    {
        self.estimated_heap_size() + std::mem::size_of::<Self>()
    }
}

impl EstimateSize for String {
    #[inline]
    fn estimated_heap_size(&self) -> usize {
        self.capacity()
    }
}

impl<T: EstimateSize> EstimateSize for Option<T> {
    #[inline]
    fn estimated_heap_size(&self) -> usize {
        if let Some(inner) = self {
            inner.estimated_heap_size()
        } else {
            0
        }
    }
}

/// SAFETY: `Bytes` can store a pointer in some cases, that may cause the size
/// of a `Bytes` be calculated more than one and when memory stats is larger than the real value.
impl EstimateSize for Bytes {
    #[inline]
    fn estimated_heap_size(&self) -> usize {
        self.len()
    }
}

impl EstimateSize for Box<str> {
    fn estimated_heap_size(&self) -> usize {
        self.len()
    }
}
