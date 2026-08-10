#![allow(dead_code)]

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct RingBuffer<T, const N: usize> {
    buffer: VecDeque<T>,
}

impl<T, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self {
            buffer: VecDeque::with_capacity(N),
        }
    }
}

impl<T, const N: usize> RingBuffer<T, N> {
    pub const CAPACITY: usize = N;

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.len() == Self::CAPACITY
    }

    /// Adds an item to the back, returning the evicted oldest item if full.
    pub fn push_back(&mut self, value: T) -> Option<T> {
        if Self::CAPACITY == 0 {
            return Some(value);
        }

        let evicted = if self.is_full() {
            self.buffer.pop_front()
        } else {
            None
        };

        self.buffer.push_back(value);
        evicted
    }

    /// Adds an item to the front, returning the evicted newest item if full.
    pub fn push_front(&mut self, value: T) -> Option<T> {
        if Self::CAPACITY == 0 {
            return Some(value);
        }

        let evicted = if self.is_full() {
            self.buffer.pop_back()
        } else {
            None
        };

        self.buffer.push_front(value);
        evicted
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.buffer.pop_front()
    }

    pub fn pop_back(&mut self) -> Option<T> {
        self.buffer.pop_back()
    }

    pub fn front(&self) -> Option<&T> {
        self.buffer.front()
    }

    pub fn back(&self) -> Option<&T> {
        self.buffer.back()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.buffer.get(index)
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }
}

impl<T, const N: usize> IntoIterator for RingBuffer<T, N> {
    type Item = T;
    type IntoIter = std::collections::vec_deque::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffer.into_iter()
    }
}
