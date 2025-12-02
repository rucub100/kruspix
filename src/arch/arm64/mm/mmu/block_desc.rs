use core::ops::{Deref, DerefMut};

use super::{BlockAndPageDescriptorAttributes, LEVEL_1_SHIFT, LEVEL_2_SHIFT};

#[repr(transparent)]
pub struct BlockDescriptor(u64);

impl BlockDescriptor {
    pub const fn new_level_1(output_addr: usize) -> Self {
        assert!(output_addr.is_multiple_of(1 << LEVEL_1_SHIFT));
        Self((output_addr as u64) | 0b01)
    }

    pub const fn new_level_2(output_addr: usize) -> Self {
        assert!(output_addr.is_multiple_of(1 << LEVEL_2_SHIFT));
        Self((output_addr as u64) | 0b01)
    }
}

impl Deref for BlockDescriptor {
    type Target = u64;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BlockDescriptor {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BlockAndPageDescriptorAttributes for BlockDescriptor {}
