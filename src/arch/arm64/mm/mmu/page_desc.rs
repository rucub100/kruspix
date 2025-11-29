use core::ops::{Deref, DerefMut};

use super::{BlockAndPageDescriptorAttributes, PAGE_SIZE};

#[repr(transparent)]
pub struct PageDescriptor(u64);

impl PageDescriptor {
    pub const fn new(output_addr: usize) -> Self {
        assert!(output_addr.is_multiple_of(PAGE_SIZE));
        Self((output_addr as u64) | 0b11)
    }
}

impl Deref for PageDescriptor {
    type Target = u64;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PageDescriptor {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BlockAndPageDescriptorAttributes for PageDescriptor {}
