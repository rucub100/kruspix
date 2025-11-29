//! MMU (Memory Management Unit) setup for ARM64 architecture.
//!
//! Page size:          4KiB
//! Page table levels:  4 (Level 0 to Level 3)

use core::arch::asm;
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use crate::mm::layout::{
    HEAP_MAP_OFFSET, IO_PERIPHERALS_MAP_OFFSET, LINEAR_MAP_OFFSET, PAGE_SIZE, USER_MAP_OFFSET,
};
use crate::mm::{alloc_page, phys_to_virt, virt_to_phys};

const PAGE_TABLE_ENTRIES: usize = const {
    assert!(size_of::<usize>() == 8);
    assert!(size_of::<u64>() == 8);
    PAGE_SIZE / size_of::<usize>()
};

const PAGE_SHIFT: usize = 12;
const LEVEL_3_SHIFT: usize = PAGE_SHIFT;
const LEVEL_2_SHIFT: usize = LEVEL_3_SHIFT + 9;
const LEVEL_1_SHIFT: usize = LEVEL_2_SHIFT + 9;
const LEVEL_0_SHIFT: usize = LEVEL_1_SHIFT + 9;

const LEVEL_0_LINEAR_INDEX: usize = level_0_index(LINEAR_MAP_OFFSET);
const LEVEL_0_HEAP_INDEX: usize = level_0_index(HEAP_MAP_OFFSET);
const LEVEL_0_IO_INDEX: usize = level_0_index(IO_PERIPHERALS_MAP_OFFSET);

trait TableDescriptorAttributes {
    const NS_TABLE: u64 = 1 << 63;
    const AP_TABLE: u64 = 11 << 61;
    const XN_TABLE: u64 = 1 << 60;
    const PXN_TABLE: u64 = 1 << 59;
}

trait BlockAndPageDescriptorAttributes: DerefMut<Target = u64> + Sized {
    const EXECUTE_NEVER: u64 = 1 << 54;
    const ACCESS_FLAG: u64 = 1 << 10;
    const SHAREABILITY: u64 = 3 << 8;
    const ATTR_INDEX: u64 = 7 << 2;

    fn set_execute_never(&mut self, xn: bool) -> &mut Self {
        if xn {
            **self |= Self::EXECUTE_NEVER;
        } else {
            **self &= !Self::EXECUTE_NEVER;
        }

        self
    }

    fn set_accessed(&mut self, accessed: bool) -> &mut Self {
        if accessed {
            **self |= Self::ACCESS_FLAG;
        } else {
            **self &= !Self::ACCESS_FLAG;
        }
        self
    }

    fn set_shareability(&mut self, attr: ShareabilityAttribute) -> &mut Self {
        **self &= !Self::SHAREABILITY;
        **self |= (attr as u64) << 8;
        self
    }

    fn set_mem_attr_index(&mut self, index: MemoryRegionAttrIndex) -> &mut Self {
        **self &= !Self::ATTR_INDEX;
        **self |= (index as u64) << 2;
        self
    }
}

enum ShareabilityAttribute {
    NonShareable = 0b00,
    OuterShareable = 0b10,
    InnerShareable = 0b11,
}

/// Memory region attribute index for `MAIR_EL1` register.
///
/// # Safety
/// This enum is derived from the `MAIR_EL1` register configuration.
/// The configuration must match the indices used here.
/// See [`crate::arch::kernel::entry`] for more details.
enum MemoryRegionAttrIndex {
    DeviceNgnRnE = 0b000,
    NormalNonCacheable = 0b001,
    NormalWriteBackNonTransientReadWriteAlloc = 0b010,
}

#[repr(C, align(4096))]
struct PageTable {
    descriptors: [u64; PAGE_TABLE_ENTRIES],
}

impl PageTable {
    fn new() -> &'static mut Self {
        unsafe {
            let table_ptr = alloc_page() as *mut PageTable;
            let table = &mut *table_ptr;
            table.descriptors.iter_mut().for_each(|d| *d = 0);

            table
        }
    }

    fn phys_addr(&self) -> usize {
        virt_to_phys(self as *const _ as usize)
    }
}

#[repr(transparent)]
struct TableDescriptor(u64);

impl TableDescriptor {
    const fn new(next_level_table_addr: usize) -> Self {
        assert!(next_level_table_addr.is_multiple_of(PAGE_SIZE));
        Self((next_level_table_addr as u64) | 0b11)
    }

    const unsafe fn next_level_table(&self) -> &mut PageTable {
        let phys_table_addr = self.0 & 0x0000_FFFF_FFFF_F000;
        let table_addr = phys_to_virt(phys_table_addr as usize);
        unsafe { &mut *(table_addr as *mut PageTable) }
    }
}

impl Deref for TableDescriptor {
    type Target = u64;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TableDescriptor {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TableDescriptorAttributes for TableDescriptor {}

#[repr(transparent)]
struct BlockDescriptor(u64);

impl BlockDescriptor {
    const LEVEL_1_ADDR_MASK: u64 = 0x0000_FFFF_FFE0_0000;
    const fn new_level_1(output_addr: usize) -> Self {
        assert!(output_addr.is_multiple_of(1 << LEVEL_1_SHIFT));
        Self((output_addr as u64) | 0b01)
    }

    const fn new_level_2(output_addr: usize) -> Self {
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

#[repr(transparent)]
struct PageDescriptor(u64);

impl PageDescriptor {
    const fn new(output_addr: usize) -> Self {
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

#[repr(transparent)]
struct InvalidDescriptor(u64);

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

enum Descriptor {
    Table(&'static mut TableDescriptor),
    Block(&'static mut BlockDescriptor),
    Page(&'static mut PageDescriptor),
    Invalid,
}

#[derive(PartialEq, Eq, Debug)]
enum TranslationLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}

impl Descriptor {
    fn from(value: u64, level: &TranslationLevel) -> Self {
        match value & 0b11 {
            0b11 => match level {
                TranslationLevel::Level0 | TranslationLevel::Level1 | TranslationLevel::Level2 => {
                    Descriptor::Table(unsafe { &mut *(value as *mut TableDescriptor) })
                }
                TranslationLevel::Level3 => {
                    Descriptor::Page(unsafe { &mut *(value as *mut PageDescriptor) })
                }
            },
            0b01 => match level {
                TranslationLevel::Level0 => {
                    panic!("Block descriptor is not valid at level 0");
                }
                TranslationLevel::Level1 | TranslationLevel::Level2 => {
                    Descriptor::Block(unsafe { &mut *(value as *mut BlockDescriptor) })
                }
                TranslationLevel::Level3 => {
                    panic!("Block descriptor is not valid at level 3");
                }
            },
            _ => Descriptor::Invalid,
        }
    }
}

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

static KERNEL_TABLE: AtomicPtr<PageTable> = AtomicPtr::new(ptr::null_mut());
static USER_TABLE: AtomicPtr<PageTable> = AtomicPtr::new(ptr::null_mut());

pub unsafe fn setup_page_tables() {
    let user_table_0 = PageTable::new();
    let kernel_table_0 = PageTable::new();

    // map first 512 GiB of user virtual address space
    let user_table_1 = PageTable::new();
    let desc_user_table_1 = TableDescriptor::new(user_table_1.phys_addr());
    user_table_0.descriptors[level_0_index(USER_MAP_OFFSET)] = *desc_user_table_1;
    for (index, desc) in user_table_1.descriptors.iter_mut().enumerate() {
        *desc = **(BlockDescriptor::new_level_1(index * (1 << LEVEL_1_SHIFT))
            .set_shareability(ShareabilityAttribute::InnerShareable)
            .set_accessed(true)
            .set_execute_never(false)
            .set_mem_attr_index(MemoryRegionAttrIndex::DeviceNgnRnE))
    }

    let kernel_table_1_linear = PageTable::new();
    let desc_kernel_table_1_linear = TableDescriptor::new(kernel_table_1_linear.phys_addr());
    let kernel_table_1_heap = PageTable::new();
    let desc_kernel_table_1_heap = TableDescriptor::new(kernel_table_1_heap.phys_addr());
    let kernel_table_1_io = PageTable::new();
    let desc_kernel_table_1_io = TableDescriptor::new(kernel_table_1_io.phys_addr());

    kernel_table_0.descriptors[LEVEL_0_LINEAR_INDEX] = *desc_kernel_table_1_linear;
    kernel_table_0.descriptors[LEVEL_0_HEAP_INDEX] = *desc_kernel_table_1_heap;
    kernel_table_0.descriptors[LEVEL_0_IO_INDEX] = *desc_kernel_table_1_io;

    for (index, desc) in kernel_table_1_linear.descriptors.iter_mut().enumerate() {
        *desc = **(BlockDescriptor::new_level_1(index * (1 << LEVEL_1_SHIFT))
            .set_shareability(ShareabilityAttribute::InnerShareable)
            .set_accessed(true)
            .set_execute_never(false)
            .set_mem_attr_index(MemoryRegionAttrIndex::NormalWriteBackNonTransientReadWriteAlloc));
    }

    unsafe {
        asm!("dsb ishst");
        asm!("msr ttbr0_el1, {}", in(reg) user_table_0.phys_addr());
        asm!("msr ttbr1_el1, {}", in(reg) kernel_table_0.phys_addr());
        asm!("tlbi vmalle1");
        asm!("dsb ish");
        asm!("isb");
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
        level_3_table.descriptors[level_3_index] = page_desc;
    }
}

#[inline(always)]
fn get_or_create_next_leve_table(
    table: &mut PageTable,
    index: usize,
    translation_level: TranslationLevel,
) -> &mut PageTable {
    let descriptor = Descriptor::from(table.descriptors[index], &translation_level);
    let table_desc = match descriptor {
        Descriptor::Invalid => {
            let new_table = PageTable::new();
            let new_desc = TableDescriptor::new(new_table.phys_addr());
            table.descriptors[index] = *new_desc;
            unsafe { &mut *(table.descriptors[index] as *mut TableDescriptor) }
        }
        Descriptor::Table(td) => td,
        _ => panic!("Unexpected descriptor type at level {translation_level:?}"),
    };

    unsafe { table_desc.next_level_table() }
}

const fn level_0_index(va: usize) -> usize {
    assert!(va.is_multiple_of(1 << LEVEL_0_SHIFT));
    (va >> LEVEL_0_SHIFT) & 0x1FF
}

const fn level_1_index(va: usize) -> usize {
    assert!(va.is_multiple_of(1 << LEVEL_1_SHIFT));
    (va >> LEVEL_1_SHIFT) & 0x1FF
}

const fn level_2_index(va: usize) -> usize {
    assert!(va.is_multiple_of(1 << LEVEL_2_SHIFT));
    (va >> LEVEL_2_SHIFT) & 0x1FF
}

const fn level_3_index(va: usize) -> usize {
    assert!(va.is_multiple_of(1 << LEVEL_3_SHIFT));
    (va >> LEVEL_3_SHIFT) & 0x1FF
}
