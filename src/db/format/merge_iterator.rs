use std::{
    cmp::Ordering,
    collections::{binary_heap::PeekMut, BinaryHeap},
};

use crate::db::error::StorageResult;

use super::{iterator::StorageIterator, key::TableKey, value::TableValue};

// Node is wrapper for StorageIterator.
pub struct Node<I: StorageIterator> {
    id: usize,
    iter: Box<I>,
}

impl<I: StorageIterator> PartialEq for Node<I> {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(std::cmp::Ordering::Equal)
    }
}

impl<I: StorageIterator> Eq for Node<I> {}

impl<I: StorageIterator> PartialOrd for Node<I> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: StorageIterator> Ord for Node<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .iter
            .key()
            .cmp(&self.iter.key())
            .then(self.id.cmp(&other.id))
    }
}

pub struct MergeIterator<I: StorageIterator> {
    /// The heap for merge sort.
    heap: BinaryHeap<Node<I>>,
    /// Current iterator.
    current: Option<Node<I>>,
}

impl<I: StorageIterator> MergeIterator<I> {
    pub fn new(iters: Vec<Box<I>>) -> Self {
        let mut heap = BinaryHeap::new();
        iters.into_iter().enumerate().for_each(|(idx, iterator)| {
            if iterator.is_valid() {
                heap.push(Node {
                    id: idx,
                    iter: iterator,
                });
            }
        });
        let current = heap.pop();
        Self { heap, current }
    }
}

impl<I> StorageIterator for MergeIterator<I>
where
    I: 'static
        + for<'a> StorageIterator<
            KeyType<'a> = TableKey<&'a [u8]>,
            ValueType<'a> = TableValue<&'a [u8]>,
        >,
{
    type KeyType<'a> = TableKey<&'a [u8]>;
    type ValueType<'a> = TableValue<&'a [u8]>;

    fn key(&self) -> Self::KeyType<'_> {
        self.current.as_ref().unwrap().iter.key()
    }

    fn value(&self) -> Self::ValueType<'_> {
        self.current.as_ref().unwrap().iter.value()
    }

    fn is_valid(&self) -> bool {
        self.current.as_ref().unwrap().iter.is_valid()
    }

    async fn next(&mut self) -> StorageResult<()> {
        let current = self.current.as_mut().unwrap();

        while let Some(mut inner_iter) = self.heap.peek_mut() {
            if inner_iter.iter.key() == current.iter.key() {
                if let Err(e) = inner_iter.iter.next().await {
                    PeekMut::pop(inner_iter);
                    return Err(e);
                }

                if !inner_iter.iter.is_valid() {
                    PeekMut::pop(inner_iter);
                }
            } else {
                break;
            }
        }

        current.iter.next().await?;

        if !current.iter.is_valid() {
            if let Some(iter) = self.heap.pop() {
                *current = iter;
            }
            return Ok(());
        }

        if let Some(mut inner_iter) = self.heap.peek_mut() {
            if *current < *inner_iter {
                std::mem::swap(&mut *inner_iter, current);
            }
        }
        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        self.heap
            .iter()
            .map(|x| x.iter.num_active_iterators())
            .sum::<usize>()
            + self
                .current
                .as_ref()
                .map(|x| x.iter.num_active_iterators())
                .unwrap_or(0)
    }
}
