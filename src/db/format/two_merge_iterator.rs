use crate::db::error::StorageResult;

use super::iterator::StorageIterator;

pub struct TwoMergeIterator<A: StorageIterator, B: StorageIterator> {
    a: A,
    b: B,
    choose_a: bool,
}

impl<
        A: 'static + StorageIterator,
        B: 'static + for<'a> StorageIterator<KeyType<'a> = A::KeyType<'a>>,
    > TwoMergeIterator<A, B>
{
    fn choose_a(a: &A, b: &B) -> bool {
        if !a.is_valid() {
            return false;
        }
        if !b.is_valid() {
            return true;
        }
        a.key() < b.key()
    }

    async fn skip_b(&mut self) -> StorageResult<()> {
        if self.a.is_valid() && self.b.is_valid() && self.b.key() == self.a.key() {
            self.b.next().await?;
        }
        Ok(())
    }

    pub async fn create(a: A, b: B) -> StorageResult<Self> {
        let mut iter = Self {
            choose_a: false,
            a,
            b,
        };
        iter.skip_b().await?;
        iter.choose_a = Self::choose_a(&iter.a, &iter.b);
        Ok(iter)
    }
}

impl<
        A: 'static + StorageIterator,
        B: 'static
            + for<'a> StorageIterator<KeyType<'a> = A::KeyType<'a>, ValueType<'a> = A::ValueType<'a>>,
    > StorageIterator for TwoMergeIterator<A, B>
{
    type KeyType<'a> = A::KeyType<'a>;
    type ValueType<'a> = A::ValueType<'a>;

    fn key(&self) -> A::KeyType<'_> {
        if self.choose_a {
            debug_assert!(self.a.is_valid());
            self.a.key()
        } else {
            debug_assert!(self.b.is_valid());
            self.b.key()
        }
    }

    fn value(&self) -> A::ValueType<'_> {
        if self.choose_a {
            self.a.value()
        } else {
            self.b.value()
        }
    }

    fn is_valid(&self) -> bool {
        if self.choose_a {
            self.a.is_valid()
        } else {
            self.b.is_valid()
        }
    }

    async fn next(&mut self) -> StorageResult<()> {
        if self.choose_a {
            self.a.next().await?;
        } else {
            self.b.next().await?;
        }
        self.skip_b().await?;
        self.choose_a = Self::choose_a(&self.a, &self.b);
        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        self.a.num_active_iterators() + self.b.num_active_iterators()
    }
}
