#![no_std]
#![no_main]

use arch::kernel::setup::setup_arch;
use mm::init_heap;

mod arch;
mod drivers;
mod kernel;
mod mm;
mod panic_handler;

#[path = "drivers/mailbox/bcm2835_mailbox.rs"]
mod bcm2835_mailbox;

#[path = "drivers/video/framebuffer.rs"]
mod framebuffer;

fn test_heap() {
    extern crate alloc;
    use alloc::boxed::Box;
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec::Vec;

    kprintln!("[kruspix] ------------------------------------------------");
    kprintln!("[kruspix] Starting Extensive Heap Test");
    kprintln!("[kruspix] ------------------------------------------------");

    // 1. Simple Box Allocation
    kprintln!("[kruspix] Test 1: Box<u32>");
    let heap_val = Box::new(42);
    assert_eq!(*heap_val, 42);
    kprintln!("[kruspix]   -> Success! Value is {}", *heap_val);

    // 2. BTreeMap (Stress Test for Small Allocations)
    // This creates hundreds of individual nodes, effectively testing your free list buckets.
    kprintln!("[kruspix] Test 2: BTreeMap (Small Allocations)");
    let mut map = BTreeMap::new();
    for i in 0..1000 {
        map.insert(i, i * 2);
    }
    assert_eq!(map.get(&500), Some(&1000));
    assert_eq!(map.get(&999), Some(&1998));
    kprintln!("[kruspix]   -> Success! Map size: {}", map.len());

    // 3. Vector Resizing (Reallocation)
    kprintln!("[kruspix] Test 3: Vec Resizing");
    let mut vec = Vec::new();
    for i in 0..1000 {
        vec.push(i); // Triggers reallocations as capacity grows
    }
    assert_eq!(vec.len(), 1000);
    assert_eq!(vec[500], 500);
    kprintln!("[kruspix]   -> Success! Vec capacity: {}", vec.capacity());

    // 4. Large Allocation (100 MB)
    kprintln!("[kruspix] Test 4: Large Allocation (100 MB contiguous)");
    let size_mb = 100;
    let size_bytes = size_mb * 1024 * 1024;

    // Allocate 100MB. This requires ~25,600 pages.
    // If your physical memory is limited, this might panic in alloc_frame().
    // Ensure QEMU has enough RAM (-m 1G).
    let mut large_buffer: Vec<u8> = Vec::with_capacity(size_bytes);

    // We explicitly write to the start and end to ensure pages are mapped
    unsafe {
        // Since with_capacity doesn't change length, we access raw pointers or just push
        // Using `vec![0; size]` would be slower due to memset.
        let ptr = large_buffer.as_mut_ptr();
        ptr.write_volatile(0xAA); // Write to first byte
        ptr.add(size_bytes - 1).write_volatile(0xBB); // Write to last byte

        assert_eq!(ptr.read_volatile(), 0xAA);
        assert_eq!(ptr.add(size_bytes - 1).read_volatile(), 0xBB);
    }

    kprintln!(
        "[kruspix]   -> Success! Allocated {} bytes at {:p}",
        size_bytes,
        large_buffer.as_ptr()
    );

    kprintln!("[kruspix] ------------------------------------------------");
    kprintln!("[kruspix] All Heap Tests Passed Successfully!");
    kprintln!("[kruspix] ------------------------------------------------");
}

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    kprintln!("\n\n\n\n\n\n[kruspix] Starting kruspix kernel...");
    setup_arch();
    init_heap();
    // TODO: memory management setup
    // + kernel heap allocator
    // + update page tables with proper mappings (advanced FDT parsing with heap)
    // TODO: interrupts/exceptions setup
    // TODO: Scheduler setup
    // TODO: SMP system setup (CPU setup)
    // TODO: Initialize other kernel modules
    // TODO: Initialize device drivers
    // TODO: setup root user space process a.k.a. init
    // TODO: Enable interrupts and start normal operation

    test_heap();

    use crate::framebuffer::{init_framebuffer, print};
    kprintln!("[kruspix] Initializing framebuffer...");
    init_framebuffer();

    kprintln!("[kruspix] Testing framebuffer print...");
    print("Hello world!");

    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}
