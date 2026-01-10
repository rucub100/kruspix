// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

pub struct RingArray<T, const N: usize> {
    buffer: [T; N],
    index: usize,
    full: bool,
}

impl<T: Copy + Default, const N: usize> RingArray<T, N> {
    pub const fn new(fill_value: T) -> Self {
        Self {
            buffer: [fill_value; N],
            index: 0,
            full: false,
        }
    }

    pub fn push(&mut self, item: T) {
        self.buffer[self.index] = item;
        self.index = (self.index + 1) % N;

        if self.full {
            return;
        }

        self.full = self.index == 0;
    }

    pub fn clear(&mut self) {
        // Note: Ignore this for performance reasons
        // self.buffer = [T::default(); N];
        self.index = 0;
        self.full = false;
    }

    pub fn iter(&self) -> RingArrayIter<'_, T, N> {
        RingArrayIter {
            buffer: &self.buffer,
            pos: if self.full { self.index } else { 0 },
            count: self.len(),
        }
    }

    pub fn drain(&mut self, dst: &mut [T]) -> usize {
        let len = self.len().min(dst.len());
        let start = if self.full { self.index } else { 0 };

        let part = (N - start).min(len);
        dst[..part].copy_from_slice(&self.buffer[start..start + part]);

        if len > part {
            dst[part..len].copy_from_slice(&self.buffer[..len - part]);
        }

        self.clear();
        len
    }

    pub fn len(&self) -> usize {
        if self.full { N } else { self.index }
    }

    pub fn is_empty(&self) -> bool {
        !self.full && self.index == 0
    }

    pub fn is_full(&self) -> bool {
        self.full
    }
}

pub struct RingArrayIter<'a, T, const N: usize> {
    buffer: &'a [T; N],
    pos: usize,
    count: usize,
}

impl<'a, T, const N: usize> Iterator for RingArrayIter<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        
        let item = &self.buffer[self.pos];
        self.pos = (self.pos + 1) % N;
        self.count -= 1;
        Some(item)
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a RingArray<T, N>
where
    T: Copy + Default,
{
    type Item = &'a T;
    type IntoIter = RingArrayIter<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
