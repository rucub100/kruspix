---
applyTo: "**/*.rs"
description: "Rust coding conventions for kernel development"
---

## File Header

Every `.rs` file must start with:
```rust
// SPDX-License-Identifier: MIT
// Copyright (c) <year> Ruslan Curbanov <info@ruslan-curbanov.de>
```

## `no_std` Rules

- Never use `std::`. Only `core::` and `alloc::`.
- `extern crate alloc;` is declared once at the crate root (`lib.rs` / `main.rs`). All modules can then use `alloc::` types (`Vec`, `Box`, `Arc`, `String`) without redeclaring it.
- Heap is only available after `init_heap()` — no heap allocation in early boot code (before that call).

## `unsafe` Requirements

- Every `pub unsafe fn` MUST have a `/// # Safety` doc comment explaining the invariants the caller must uphold.
- Every `unsafe { ... }` block MUST have a `// SAFETY:` comment on the line immediately before it explaining why the operation is sound.
- Never use bare `unsafe {}` without a comment. No exceptions.

## Statics and Globals

Use the correct pattern for each use case:

| Use case | Pattern |
|----------|---------|
| Write-once singleton (e.g., SCHEDULER) | `static X: OnceLock<T> = OnceLock::new();` |
| Late-initialized mutable state | `static X: SpinLock<Option<T>> = SpinLock::new(None);` |
| Shared list | `static X: SpinLock<Vec<T>> = SpinLock::new(Vec::new());` |
| Simple counter/flag | `static X: AtomicUsize = AtomicUsize::new(0);` |
| Driver static (must be `const`-initialized) | `pub static DRIVER: MyDriver = MyDriver::new();` |

- **Never use `static mut`.** It is highly unsafe and an anti-pattern. Use the safe wrappers listed above instead.
- Every struct used as a `static` MUST have a `const fn new()` constructor.
- To get a `&'static T` from a heap-allocated value, use `Box::leak(Box::new(value))`.

## Locking — `lock()` vs `lock_irq()`

- Use `lock()` when the lock is only ever held in task/thread context (no ISR contention).
- Use `lock_irq()` when the lock is accessed from an IRQ handler OR from code that may run while an IRQ is pending. This disables IRQ/FIQ before acquiring.
- Never call `lock()` on a lock that an ISR also acquires — this will deadlock.
- For critical sections without a specific lock, use `without_irq_fiq(|| { ... })`.

## MMIO (Memory-Mapped I/O)

- Define register offsets as `const REG_NAME_OFFSET: usize = 0x...;`
- Compute the register address as: `(base_va + REG_NAME_OFFSET) as *mut u32`
- Always use `core::ptr::read_volatile` / `core::ptr::write_volatile`. Never dereference MMIO pointers directly.
- For multi-register MMIO synchronization across cores, use `with_addr_lock(register_va, || { ... })` from `crate::kernel::sync`.

## ARM Inline Assembly

- Use `core::arch::asm!()` for inline assembly.
- Use `#[unsafe(naked)]` + `core::arch::naked_asm!()` for naked functions.
- Always add an `isb` (Instruction Synchronization Barrier) after writing to ARM system registers (e.g., `msr cntv_ctl_el0, ...`). Use: `unsafe { core::arch::asm!("isb", options(nomem, nostack)); }`
- Use `core::hint::spin_loop()` inside busy-wait loops (not a hand-written `nop`).

## `#[repr(C)]` and ABI

- Use `#[repr(C)]` on structs that are shared with assembly code (e.g., `ArchContext`).
- Use `#[unsafe(no_mangle)] pub extern "C" fn ...` for functions called from assembly or C (e.g., `start_kernel`, `global_irq_dispatch`).

## Error Handling

- Driver init functions return `Result<(), DriverInitError>`.
- Use `ok_or(DriverInitError::Xxx)?` to propagate errors — never `.unwrap()` or `.expect()` in driver paths.
- Use `Err(DriverInitError::Retry)` when a dependency (e.g., interrupt controller) is not yet registered.
- Use `Err(DriverInitError::ToDo)` for unimplemented driver paths.
- `unwrap()` / `expect()` / `panic!()` are only acceptable during unrecoverable early-boot failures.

## Shared Device Ownership

- Devices are reference-counted with `Arc<T>`.
- Device traits (`Device`, `PlatformDriver`, `InterruptHandler`, `InputDevice`, `OutputDevice`, `Timer`, `Alarm`) must be implemented on the concrete device struct.
- Store devices in a `DriverRegistry<T>`. Register with `registry.add_device(path, arc_dev)`.
- Expose devices to the rest of the kernel via `DEVICE_MANAGER` or trait-specific global lists (e.g., `GLOBAL_TIMERS`).

## Logging

- Use `kprint!` and `kprintln!` for all kernel log output — never `print!`/`println!`.
- Log format convention: `kprintln!("[INFO] ...")`, `kprintln!("[WARNING] ...")`, `kprintln!("[ERROR] ...")`.

## Module Conventions

- Mark subsystem-internal APIs `pub(super)` or `pub(crate)` — only expose publicly what needs to be public.
- `init()` functions for kernel modules are `pub(super) fn init() -> Result<(), E>` called from `kernel::init_modules()`.
