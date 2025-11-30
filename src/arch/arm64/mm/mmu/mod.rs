//! MMU (Memory Management Unit) setup for ARM64 architecture.
//!
//! Page size:          4KiB
//! Page table levels:  4 (Level 0 to Level 3)

use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use crate::mm::layout::{
    HEAP_MAP_OFFSET, IO_PERIPHERALS_MAP_OFFSET, LINEAR_MAP_OFFSET, USER_MAP_OFFSET,
};

use attr::{BlockAndPageDescriptorAttributes, MemoryRegionAttrIndex, ShareabilityAttribute};
use block_desc::BlockDescriptor;
use desc::Descriptor;
use page_desc::PageDescriptor;
use page_table::PageTable;
use table_desc::TableDescriptor;
use tlb::{invalidate_all, invalidate_by_va_all_asid_inner_shareable};

mod attr;
mod block_desc;
mod desc;
mod page_desc;
mod page_table;
mod table_desc;
mod tlb;

pub const PAGE_SIZE: usize = 4096;
const PAGE_TABLE_ENTRIES: usize = const {
    assert!(size_of::<usize>() == 8);
    assert!(size_of::<u64>() == 8);
    PAGE_SIZE / size_of::<usize>()
};
const BLOCK_SIZE: usize = PAGE_SIZE * PAGE_TABLE_ENTRIES;
const ADDR_MASK: u64 = 0x0000_ffff_ffff_f000;

const PAGE_SHIFT: usize = 12;
const LEVEL_3_SHIFT: usize = PAGE_SHIFT;
const LEVEL_2_SHIFT: usize = LEVEL_3_SHIFT + 9;
const LEVEL_1_SHIFT: usize = LEVEL_2_SHIFT + 9;
const LEVEL_0_SHIFT: usize = LEVEL_1_SHIFT + 9;

const LEVEL_0_LINEAR_INDEX: usize = level_0_index(LINEAR_MAP_OFFSET);
const LEVEL_0_HEAP_INDEX: usize = level_0_index(HEAP_MAP_OFFSET);
const LEVEL_0_IO_INDEX: usize = level_0_index(IO_PERIPHERALS_MAP_OFFSET);

#[derive(Eq, PartialEq, Debug)]
enum VirtualAddressSpace {
    User,
    Kernel,
}

impl From<usize> for VirtualAddressSpace {
    #[inline(always)]
    fn from(value: usize) -> Self {
        if value >= 0xffff_0000_0000_0000 {
            VirtualAddressSpace::Kernel
        } else {
            VirtualAddressSpace::User
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
enum TranslationLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}

static KERNEL_TABLE: AtomicPtr<PageTable> = AtomicPtr::new(ptr::null_mut());
static USER_TABLE: AtomicPtr<PageTable> = AtomicPtr::new(ptr::null_mut());

pub unsafe fn setup_page_tables() {
    let user_table_0 = PageTable::new();
    let kernel_table_0 = PageTable::new();

    // map first 512 GiB of user virtual address space
    let user_table_1 = PageTable::new();
    for (index, desc) in user_table_1.iter_mut().enumerate() {
        *desc = **(BlockDescriptor::new_level_1(index * (1 << LEVEL_1_SHIFT))
            .set_shareability(ShareabilityAttribute::InnerShareable)
            .set_accessed(true)
            .set_execute_never(false)
            .set_mem_attr_index(MemoryRegionAttrIndex::DeviceNgnRnE))
    }
    let desc_user_table_1 = TableDescriptor::new(user_table_1.phys_addr());
    user_table_0.set_descriptor(level_0_index(USER_MAP_OFFSET), *desc_user_table_1);

    let kernel_table_1_linear = PageTable::new();
    let desc_kernel_table_1_linear = TableDescriptor::new(kernel_table_1_linear.phys_addr());
    let kernel_table_1_heap = PageTable::new();
    let desc_kernel_table_1_heap = TableDescriptor::new(kernel_table_1_heap.phys_addr());
    let kernel_table_1_io = PageTable::new();
    let desc_kernel_table_1_io = TableDescriptor::new(kernel_table_1_io.phys_addr());

    kernel_table_0.set_descriptor(LEVEL_0_LINEAR_INDEX, *desc_kernel_table_1_linear);
    kernel_table_0.set_descriptor(LEVEL_0_HEAP_INDEX, *desc_kernel_table_1_heap);
    kernel_table_0.set_descriptor(LEVEL_0_IO_INDEX, *desc_kernel_table_1_io);

    for (index, desc) in kernel_table_1_linear.iter_mut().enumerate() {
        *desc = **(BlockDescriptor::new_level_1(index * (1 << LEVEL_1_SHIFT))
            .set_shareability(ShareabilityAttribute::InnerShareable)
            .set_accessed(true)
            .set_execute_never(false)
            .set_mem_attr_index(MemoryRegionAttrIndex::NormalWriteBackNonTransientReadWriteAlloc));
    }

    unsafe {
        invalidate_all(user_table_0.phys_addr(), kernel_table_0.phys_addr());
    }

    KERNEL_TABLE.store(kernel_table_0, Ordering::Release);
    USER_TABLE.store(user_table_0, Ordering::Release);
}

pub fn map_page(va: usize, pa: usize) {
    if VirtualAddressSpace::from(va) == VirtualAddressSpace::User {
        todo!()
    }

    let level_0_index = level_0_index(va);
    let level_1_index = level_1_index(va);
    let level_2_index = level_2_index(va);
    let level_3_index = level_3_index(va);

    let page_desc = match level_0_index {
        LEVEL_0_LINEAR_INDEX => {
            panic!("Mapping pages in linear region is not supported");
        }
        LEVEL_0_HEAP_INDEX => {
            **(PageDescriptor::new(pa)
                .set_shareability(ShareabilityAttribute::InnerShareable)
                .set_accessed(true)
                .set_execute_never(true)
                .set_mem_attr_index(
                    MemoryRegionAttrIndex::NormalWriteBackNonTransientReadWriteAlloc,
                ))
        }
        LEVEL_0_IO_INDEX => {
            **(PageDescriptor::new(pa)
                .set_shareability(ShareabilityAttribute::InnerShareable)
                .set_accessed(true)
                .set_execute_never(true)
                .set_mem_attr_index(MemoryRegionAttrIndex::DeviceNgnRnE))
        }

        _ => {
            panic!("Unsupported virtual address region for mapping: {:#x}", va);
        }
    };

    unsafe {
        let level_0_table = &mut *KERNEL_TABLE.load(Ordering::Acquire);
        let level_1_table =
            get_or_create_next_leve_table(level_0_table, level_0_index, TranslationLevel::Level0);
        let level_2_table =
            get_or_create_next_leve_table(level_1_table, level_1_index, TranslationLevel::Level1);

        let level_3_table =
            get_or_create_next_leve_table(level_2_table, level_2_index, TranslationLevel::Level2);
        level_3_table.set_descriptor(level_3_index, page_desc);

        invalidate_by_va_all_asid_inner_shareable(va);
    }
}

#[inline(always)]
fn get_or_create_next_leve_table(
    table: &mut PageTable,
    index: usize,
    translation_level: TranslationLevel,
) -> &mut PageTable {
    let descriptor = Descriptor::from(table.get_descriptor(index), &translation_level);
    let table_desc = match descriptor {
        Descriptor::Invalid => {
            let new_table = PageTable::new();
            let new_desc = TableDescriptor::new(new_table.phys_addr());
            table.set_descriptor(index, *new_desc);
            table.as_table_descriptor(index)
        }
        Descriptor::Table => table.as_table_descriptor(index),
        _ => panic!("Unexpected descriptor type at level {translation_level:?}"),
    };

    unsafe { table_desc.next_level_table() }
}

const fn level_0_index(va: usize) -> usize {
    (va >> LEVEL_0_SHIFT) & 0x1FF
}

const fn level_1_index(va: usize) -> usize {
    (va >> LEVEL_1_SHIFT) & 0x1FF
}

const fn level_2_index(va: usize) -> usize {
    (va >> LEVEL_2_SHIFT) & 0x1FF
}

const fn level_3_index(va: usize) -> usize {
    (va >> LEVEL_3_SHIFT) & 0x1FF
}
