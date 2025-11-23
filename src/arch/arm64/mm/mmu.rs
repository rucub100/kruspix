//! MMU (Memory Management Unit) setup for ARM64 architecture.
//!
//! Page size:          4KiB
//! Page table levels:  4 (Level 0 to Level 3)

use core::arch::asm;
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use crate::mm::layout::{
    HEAP_MAP_OFFSET, IO_PERIPHERALS_MAP_OFFSET, LEVEL_0_SHIFT, LEVEL_1_SHIFT, LEVEL_2_SHIFT,
    LEVEL_3_SHIFT, LINEAR_MAP_OFFSET, PAGE_SIZE, PAGE_TABLE_ENTRIES, USER_MAP_OFFSET,
};
use crate::mm::{alloc_frame, phys_to_virt, virt_to_phys};

#[repr(C, align(4096))]
struct PageTable {
    descriptors: [u64; PAGE_TABLE_ENTRIES],
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

struct TableDescriptor {
    value: u64,
}

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

impl<T> BlockAndPageDescriptorAttributes for T where T: DerefMut<Target = u64> {}

#[repr(transparent)]
struct BlockDescriptor(u64);

#[repr(transparent)]
struct PageDescriptor(u64);

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

static KERNEL_TABLE: AtomicPtr<PageTable> = AtomicPtr::new(ptr::null_mut());
static USER_TABLE: AtomicPtr<PageTable> = AtomicPtr::new(ptr::null_mut());

impl PageTable {
    fn new() -> &'static mut Self {
        unsafe {
            let phys_table_ptr = alloc_frame();
            let phys_table_addr = phys_table_ptr as usize;
            let table_addr = phys_to_virt(phys_table_addr);
            let table_ptr = table_addr as *mut PageTable;
            let table = &mut *table_ptr;
            table.descriptors.iter_mut().for_each(|d| *d = 0);

            table
        }
    }

    fn phys_addr(&self) -> usize {
        virt_to_phys(self as *const _ as usize)
    }
}

impl TableDescriptor {
    const fn new(next_level_table_addr: usize) -> Self {
        assert!(next_level_table_addr.is_multiple_of(PAGE_SIZE));
        Self {
            value: (next_level_table_addr as u64) | 0b11,
        }
    }
}

impl BlockDescriptor {
    const fn new_level_1(output_addr: usize) -> Self {
        assert!(output_addr.is_multiple_of(1 << LEVEL_1_SHIFT));
        Self((output_addr as u64) | 0b01)
    }

    const fn new_level_2(output_addr: usize) -> Self {
        assert!(output_addr.is_multiple_of(1 << LEVEL_2_SHIFT));
        Self((output_addr as u64) | 0b01)
    }
}

pub unsafe fn setup_page_tables() {
    let user_table_0 = PageTable::new();
    let kernel_table_0 = PageTable::new();

    // map first 512 GiB of user virtual address space
    let user_table_1 = PageTable::new();
    let desc_user_table_1 = TableDescriptor::new(user_table_1.phys_addr());
    user_table_0.descriptors[level_0_index(USER_MAP_OFFSET)] = desc_user_table_1.value;
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

    let linear_index = level_0_index(LINEAR_MAP_OFFSET);
    let heap_index = level_0_index(HEAP_MAP_OFFSET);
    let io_index = level_0_index(IO_PERIPHERALS_MAP_OFFSET);
    kernel_table_0.descriptors[linear_index] = desc_kernel_table_1_linear.value;
    kernel_table_0.descriptors[heap_index] = desc_kernel_table_1_heap.value;
    kernel_table_0.descriptors[io_index] = desc_kernel_table_1_io.value;

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
