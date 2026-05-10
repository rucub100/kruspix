---
name: Rust Kernel Expert
description: Triggers whenever modifying, creating, or refactoring Rust code in any directory.
---

# Rust Kernel Expert

You are working inside **Kruspix** — a bare-metal `#![no_std]` OS kernel for ARM64 (Raspberry Pi 3). Before writing any code, internalize the architectural rules in `AGENTS.md` and the per-file conventions in `.github/instructions/rust.instructions.md`.

---

## How to Write a New Platform Driver

A driver consists of two structs: one for the **device instance** and one for the **driver singleton**.

### Step 1 — Create the device struct

```rust
struct MyDevice {
    id: String,
    reg_base: usize,           // mapped virtual address from map_io_region()
    // ... device state protected by SpinLock as needed
}

impl Device for MyDevice {
    fn id(&self) -> &str { self.id.as_str() }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        // one-time global initialization (parse DT props, register IRQ handler, etc.)
        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        // per-core initialization (called during SMP boot, if applicable)
        Ok(())
    }
}
```

### Step 2 — Create the driver singleton

```rust
pub struct MyDriver {
    dev_registry: DriverRegistry<MyDevice>,
}

impl MyDriver {
    pub const fn new() -> Self {   // MUST be const for static initialization
        Self { dev_registry: DriverRegistry::new() }
    }
}

impl PlatformDriver for MyDriver {
    fn compatible(&self) -> &[&str] {
        &["vendor,chip-name"]   // must match DT "compatible" string
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        // 1. Parse required DT properties (reg, interrupts, clocks, etc.)
        let (phys_addr, size) = node
            .resolve_phys_address_and_length(0)
            .ok_or(DriverInitError::DeviceTreeError)?;

        // 2. Map MMIO region
        let va = map_io_region(phys_addr, size);

        // 3. Construct device
        let dev = Arc::new(MyDevice::new(node.path(), va));

        // 4. Register IRQ handler (if needed). Return Retry if IRQ system not ready yet.
        let virq = resolve_virq(node, 0).map_err(|_| DriverInitError::Retry)?;
        register_handler(virq, dev.clone()).map_err(|_| DriverInitError::Retry)?;
        enable_irq(virq).map_err(|_| DriverInitError::ToDo)?;

        // 5. Call global_setup
        dev.clone().global_setup(node)?;

        // 6. Register device
        self.dev_registry.add_device(node.path(), dev);
        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: MyDriver = MyDriver::new();
```

### Step 3 — Register in `src/drivers/mod.rs`

Add `mod my_driver;` at the top, then add `&my_driver::my_module::DRIVER` to `PLATFORM_DRIVERS`.

### Step 4 — Implement device traits as needed

Implement `InterruptHandler`, `InputDevice`, `OutputDevice`, `Timer`, `Alarm`, etc. on the device struct and register with the appropriate subsystem (`register_handler`, `register_input`, `register_output`, `register_global_timer`, etc.).

---

## How to Register an IRQ Handler

```rust
// 1. Resolve the virtual IRQ number from the device tree node
let virq = resolve_virq(node, 0)             // index 0 = first interrupt in DT
    .map_err(|_| DriverInitError::Retry)?;   // Retry if interrupt controller not ready

// 2. Register a handler (Arc<dyn InterruptHandler> OR any Fn(u32) + Send + Sync closure)
register_handler(virq, dev.clone())
    .map_err(|_| DriverInitError::Retry)?;   // Retry if already registered (race)

// 3. Unmask the interrupt at the controller
enable_irq(virq).map_err(|_| DriverInitError::ToDo)?;
```

The `InterruptHandler` trait:
```rust
impl InterruptHandler for MyDevice {
    fn handle_irq(&self, _virq: u32) {
        // called from IRQ context — keep short, no sleeping, use lock() not lock_irq()
        // (IRQs are already disabled in the dispatch path)
    }
}
```

---

## How to Add a Shell Command

```rust
use crate::kernel::shell::{ShellCommand, register_command};

// Inside a module's init() function:
shell::register_command(ShellCommand::new("mycommand", "Description", |_shell, args| {
    // args[0] is the command name
    if let Some(terminal) = get_system_terminal() {
        terminal.write(b"Hello from mycommand\r\n");
    }
}));
```

---

## How to Add a New Kernel Module

1. Create `src/kernel/mymodule.rs`.
2. Declare it in `src/kernel/mod.rs`: `pub mod mymodule;`
3. Add an `pub(super) fn init() -> Result<(), MyError>` in the module.
4. Call it from `kernel::init_modules()` in `src/kernel/mod.rs`.

---

## Common Patterns

### Lazy-initialized global state
```rust
static MY_STATE: SpinLock<Option<MyType>> = SpinLock::new(None);

// Initialize once:
MY_STATE.lock().replace(MyType::new());

// Access:
if let Some(state) = MY_STATE.lock().as_ref() { ... }
```

### Write-once singleton (for subsystems initialized exactly once)
```rust
static MY_SINGLETON: OnceLock<MyType> = OnceLock::new();

// Set (returns Err if already set):
MY_SINGLETON.set(MyType::new()).ok();

// Get:
if let Some(val) = MY_SINGLETON.get() { ... }
```

### Heap-allocated `&'static T` (for per-core data)
```rust
let data = Box::leak(Box::new(MyData::new()));  // 'static lifetime
unsafe { set_local(data); }
```

### MMIO register access
```rust
use crate::kernel::sync::with_addr_lock;

const MY_REG_OFFSET: usize = 0x10;
let reg_ptr = (self.reg_base + MY_REG_OFFSET) as *mut u32;

with_addr_lock(reg_ptr as usize, || {
    // SAFETY: reg_ptr is a valid MMIO virtual address obtained from map_io_region().
    let val = unsafe { core::ptr::read_volatile(reg_ptr) };
    // SAFETY: reg_ptr is a valid MMIO virtual address obtained from map_io_region().
    unsafe { core::ptr::write_volatile(reg_ptr, val | SOME_BIT) };
});
```

### Early console driver (pre-heap output)
Implement `early_init(&'static self, fdt: &Fdt, path: &str)` on the driver static, call `register_early_console(self)`. Use only static state — no heap allocation here.

---

## Critical Pitfalls

- **Do not call `lock()` on a `SpinLock` that an ISR also acquires** — use `lock_irq()` or the ISR will deadlock against the task holding the lock.
- **Do not allocate on the heap before `init_heap()`** — this is before `setup_page_tables()` returns.
- **Do not read/write MMIO before `map_io_region()`** — the virtual address does not exist yet.
- **Never call `sleep()` or `yield_task()` from IRQ context** — ISR handlers must be non-blocking.
- **`without_irq_fiq` is not reentrant** — nesting is safe but keep critical sections minimal.
- **Add `isb` after every ARM system register write** — without it, the CPU may speculatively use the old value. Use: `unsafe { core::arch::asm!("isb", options(nomem, nostack)); }`
- **`map_page()` for user VA space calls `todo!()`** — user space mapping is not yet implemented.
- **`sleep()` is currently a busy-loop** — do not rely on it for precise timing in production paths.
- **The heap allocator does NOT recover freed multi-page blocks** — large allocations (>4KiB) are effectively permanent.
