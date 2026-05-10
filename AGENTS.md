# Kruspix OS - AI Agent Context & Guidelines

## 🤖 Introduction
You are an expert Systems Programmer and OS Kernel Developer assisting with **Kruspix**, an educational, bare-metal operating system kernel written in Rust targeting the Raspberry Pi (ARM64).

When generating code, analyzing bugs, or suggesting architectural changes, refer strictly to the context and rules defined in this document.

---

## 🎯 Project Overview
- **Name:** Kruspix
- **Goal:** A hands-on, bare-metal OS designed to teach OS concepts from the ground up on Raspberry Pi hardware.
- **Target Architecture:** `aarch64-unknown-none` (ARM64).
- **Supported Hardware:** Raspberry Pi 3 Model B v1.2 (BCM2837) — the only hardware currently tested and confirmed working on real hardware.
- **Planned Hardware:** Raspberry Pi 2 Model B v1.2 and Pi 4 Model B (BCM2711) are planned targets but not yet supported.
- **Environment:** `#![no_std]` and `#![no_main]`.
- **Language Edition:** Rust 2024.

---

## 📂 Codebase Architecture
The source code is modularized into distinct subsystems:

- **`src/main.rs`**: The kernel entry point (`start_kernel`). Orchestrates early boot: arch setup, paging, heap, devicetree, drivers, and scheduler initiation.
- **`src/lib.rs`**: Library crate root. Re-exports all top-level modules (`arch`, `common`, `drivers`, `kernel`, `mm`, `panic_handler`). Does not contain any boot logic.
- **`src/panic_handler.rs`**: Defines the `#[panic_handler]`. On panic it prints the file/line/message via `kprintln!` and halts with `wait_for_event()`. Modify this file if you need to change the panic output format.
- **`src/arch/arm64/`**: Architecture-specific logic for ARMv8/AArch64.
    - `kernel/`: Low-level CPU setup (`setup.rs`) and exception vector table entry (`entry.rs`).
    - `mm/mmu/`: MMU implementation — page table management, descriptors, TLB, and mapping helpers (e.g., `map_page`, `setup_page_tables`).
    - `cpu.rs`: CPU control primitives (enable/disable IRQ/FIQ, `wait_for_interrupt`, context switching). Per-CPU local storage is implemented via the ARM64 `tpidr_el1` system register — `set_local<T>()` stores a pointer to a core-local struct at boot, and `get_local<T>()` retrieves it without any locking, enabling fast lock-free access to per-core state. This is the foundation for `init_local_data()` and `get_local_data()`.
- **`src/kernel/`**: Core OS services.
    - `sched.rs`: Task creation, the task scheduler queue, and `start_sched`. Implements a **preemptive round-robin** scheduler: a hardware timer alarm fires every 10 ms, its IRQ handler sets a schedule flag in per-CPU data (`get_local_data().set_schedule_flag()`), and `global_irq_dispatch` in `irq.rs` checks that flag at the end of every IRQ and calls `yield_task()` to trigger a context switch.
    - `cpu.rs`: Per-core local data initialization.
    - `devicetree/`: FDT (Flattened Device Tree) parsing.
    - `sync/`: Custom synchronization primitives (`SpinLock`, `OnceLock`) and `without_irq_fiq`. Also exposes `with_addr_lock(addr, || { ... })`, which selects one of 256 spinlocks using a Fibonacci hash of the given address — use this for fine-grained MMIO register synchronization to avoid contention on a single global lock.
    - `shell.rs`, `terminal.rs`, `console.rs`: Kernel shell and terminal subsystem. `terminal.rs` implements a `SystemTerminal` with a **line discipline** (echo, backspace handling, newline detection) that notifies registered `LineListener`s when a complete line is received — call `terminal.poll()` periodically from a task to drive input processing. `console.rs` is the lower-level print backend for `kprintln!`: output is written to a 4096-byte ring buffer (`BOOT_CONSOLE`) before any driver is ready; `register_early_console()` drains that buffer to the first available console and takes it live; `register_console()` later replaces the early console with a full system console.
    - `clk.rs`, `irq.rs`, `rng.rs`, `time.rs`, `watchdog.rs`, `power.rs`: Kernel-level abstractions over hardware subsystems. `irq.rs` in particular owns the global IRQ domain and handler table — use `resolve_virq(node, index)` to translate a device tree interrupt specifier to a virtual IRQ number, `register_handler(virq, handler)` to attach a handler (`Arc<dyn InterruptHandler>` or any `Fn(u32) + Send + Sync` closure), and `enable_irq(virq)` to unmask it at the controller level.
    - `print.rs`: Implements the `kprint!` / `kprintln!` macros.
- **`src/mm/`**: Physical memory management. Contains:
    - `frame_allocator.rs`: Physical page frame allocator (bitmap-based).
    - `heap_allocator.rs`: Kernel heap allocator; exposes `init_heap`.
    - `layout.rs`: Virtual address space layout constants (e.g., `LINEAR_MAP_OFFSET`, `IO_PERIPHERALS_MAP_OFFSET`).
    - `memory.rs`: Available memory calculation.
    - Helper functions: `virt_to_phys`, `phys_to_virt`, `alloc_page`, `dealloc_page`, `map_io_region`.
    - **Note:** Virtual memory mappings (page tables, MMU) live in `src/arch/arm64/mm/mmu/`, not here.
- **`src/drivers/`**: Device drivers structured around a Platform Driver model. Drivers are initialized based on Device Tree nodes. Registered drivers include: interrupt controllers, clocks, timers, watchdog, RNG, UART (serial), mailbox, and firmware (syscon). **Note:** The following driver directories exist but are not yet implemented or registered: `spi/`, `bluetooth/`, `display/`, `dma_controller/`, `ethernet/`, `mmc/`, `pinctrl/`, `usb/`, `wifi/`.
- **`src/common/`**: General utilities and data structures (e.g., `ring_array.rs`, `hash.rs`).
- **`src/fs/`, `src/net/`, `src/ipc/`, `src/init/`**: Stub/placeholder directories for planned future subsystems (filesystem, networking, IPC, init system). Currently empty — do not implement into these without explicit instruction.

---

## 🛠️ Tech Stack & Tooling
- **Compiler:** `rustc` with `aarch64-unknown-none` target, using the **stable** channel (see `rust-toolchain.toml`). Do NOT use nightly-only features or attributes.
- **Build Tools:** `cargo-binutils` (specifically `cargo objcopy` to generate the raw binary image).
- **Emulation:** QEMU for local testing.
- **Debugging:** JTAG via `openocd` / GDB.
- **Memory Allocation:** Custom `alloc` crate usage is permitted as a heap allocator is initialized during boot.

---

## 🚦 Key Conventions & Rules for AI Agents

1. **Strictly `#![no_std]`:**
    - NEVER import from `std::`.
    - Rely solely on `core::` and `alloc::`.
    - String manipulation and heap structures must use `alloc::string::String`, `alloc::vec::Vec`, `alloc::boxed::Box`, and `alloc::sync::Arc`.

2. **Concurrency & Synchronization:**
    - Standard threading (`std::thread`) is unavailable.
    - Use `crate::kernel::sync::SpinLock` for data protection.
    - For logic that must not be interrupted, execute it within `without_irq_fiq(|| { ... })` to locally disable ARM IRQ/FIQ exceptions.

3. **Memory Safety & Unsafe Rust:**
    - Bare metal programming requires `unsafe` blocks for hardware interactions (e.g., writing to memory-mapped IO registers, context switching, inline assembly).
    - **Rule:** Every `unsafe` block or function MUST have a corresponding `/// # Safety` doc comment explaining why the operation is memory-safe or what invariants the caller must uphold.

4. **Hardware Interactions (MMIO):**
    - Peripheral interactions should map memory properly using `map_io_region`.
    - Rely on `core::ptr::read_volatile` and `core::ptr::write_volatile` for reading/writing hardware registers.
    - For synchronizing access to a specific MMIO register address across cores or drivers, use `with_addr_lock(register_va, || { ... })` from `crate::kernel::sync` instead of a global lock. It hashes the address with Fibonacci hashing to select one of 256 fine-grained spinlocks, minimizing contention.

5. **Driver Development:**
    - Drivers must implement the `PlatformDriver` trait and declare their supported Device Tree "compatible" strings via the `compatible()` method.
    - Matching a driver to a device tree node is done automatically by the framework (`init_platform_drivers`). A driver's `try_init()` receives an already-matched node.
    - **Dependency handling via retry:** if `try_init()` depends on a subsystem not yet ready (e.g., an interrupt controller not yet registered), return `Err(DriverInitError::Retry)`. The framework iterates uninitialized drivers in a loop, retrying until no further progress is made — no explicit dependency graph is needed.
    - New drivers must be registered in `PLATFORM_DRIVERS` in `src/drivers/mod.rs`.
    - Initialized device instances should be stored in a `DriverRegistry<T>` and made accessible via the `DEVICE_MANAGER` global (`src/drivers/mod.rs`). Other subsystems retrieve devices through `DEVICE_MANAGER.get_device(path)`.

6. **Error Handling & Panics:**
    - The kernel is built with `panic = "abort"`.
    - Avoid `unwrap()` or `expect()` in standard driver paths; return a `Result` or `DriverInitError` instead. Panic is only acceptable during unrecoverable early-boot failures.
    - Use `kprint!` and `kprintln!` macros for kernel logging over the initialized UART console.

---

## 🚀 Entry Point Flow (`start_kernel`)
When modifying initialization logic, preserve this sequence:
1. `setup_arch()` - Reads the FDT address and initializes physical memory layout. (Note: exception vector tables and EL transition are set up in `entry.rs` assembly, before `start_kernel` is called.)
2. `setup_page_tables()` & `init_heap()` - Virtual memory and dynamic allocation.
3. `init_local_data()` - Per-core structures.
4. `init_devicetree()` - Parse FDT.
5. `init_platform_drivers()` - Load drivers from DTBs.
6. `local_enable_irq_fiq()` - Turn on interrupts.
7. `init_modules()` - Subsystems (terminal, time, rng, watchdog, scheduler).
8. `add_task(...)` -> `start_sched()` - Enter multi-tasking.