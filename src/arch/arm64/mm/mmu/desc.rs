use core::ops::{Deref, DerefMut};

use super::TranslationLevel;
use super::block_desc::BlockDescriptor;
use super::page_desc::PageDescriptor;
use super::table_desc::TableDescriptor;

pub enum Descriptor {
    Table,
    Block,
    Page,
    Invalid,
}

impl Descriptor {
    pub fn from(value: u64, level: &TranslationLevel) -> Self {
        match value & 0b11 {
            0b11 => match level {
                TranslationLevel::Level0 | TranslationLevel::Level1 | TranslationLevel::Level2 => {
                    Descriptor::Table
                }
                TranslationLevel::Level3 => Descriptor::Page,
            },
            0b01 => match level {
                TranslationLevel::Level0 => {
                    panic!("Block descriptor is not valid at level 0");
                }
                TranslationLevel::Level1 | TranslationLevel::Level2 => Descriptor::Block,
                TranslationLevel::Level3 => {
                    panic!("Block descriptor is not valid at level 3");
                }
            },
            _ => Descriptor::Invalid,
        }
    }
}

#[repr(transparent)]
pub struct InvalidDescriptor(u64);

impl Deref for InvalidDescriptor {
    type Target = u64;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for InvalidDescriptor {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
