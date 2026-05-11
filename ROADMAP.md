# Roadmap

## Vision

The ultimate goal of this project is to have a real userspace process running on top of Kruspix,
demonstrating that the entire kernel stack works end-to-end on real hardware. The concrete example
I have in mind: a Tetris game rendering on an HDMI-connected TV, with audio and input from a USB
joystick &ndash; all running bare-metal on a Raspberry Pi. This is aspirational. The milestones
below are not explicitly sequenced to reach this goal, but the feature areas collectively point in
that direction.

---

## Feature Areas

### Boot & Platform Init

- [x] CPU bring-up and EL3/EL2 → EL1 exception level transition
- [x] Exception vector table
- [x] Linker script and kernel binary layout
- [x] MMU setup with page tables and TLB management
- [x] Flattened Device Tree (FDT) parsing
- [x] Per-CPU local storage via `tpidr_el1`

### Memory Management

- [x] Physical page frame allocator (bitmap-based)
- [x] Virtual address space layout (linear map, I/O peripherals map)
- [x] Kernel heap allocator
- [x] MMIO region mapping (`map_io_region`)
- [ ] Buddy allocator (replace bitmap-based frame allocator)
- [ ] Manage all available free physical memory regions

### Interrupts & Timers

- [x] BCM2836 local interrupt controller
- [x] BCM2836 ARM control interrupt controller
- [x] Virtual IRQ domain (`resolve_virq`, `register_handler`, `enable_irq`)
- [x] ARM architected timer
- [x] BCM2835 system timer

### Core Kernel Services

- [x] Preemptive round-robin scheduler (10 ms time slice via timer alarm)
- [x] Task creation and termination
- [x] Per-CPU data (`init_local_data` / `get_local_data`)
- [x] Synchronization primitives: `SpinLock`, `OnceLock`, `without_irq_fiq`, `with_addr_lock`
- [x] Kernel logging: `kprint!` / `kprintln!` with ring buffer + UART console
- [x] Power management and watchdog abstraction
- [x] Clock and time abstractions

### Platform Drivers

- [x] Platform driver model: DTB-based matching with retry-on-dependency
- [x] BCM2835 AUX Mini UART (serial console)
- [x] Fixed clock + BCM2835 AUX clock controller
- [x] BCM2835 watchdog
- [x] BCM2835 RNG
- [x] BCM2835 mailbox (IPC with GPU firmware)
- [x] Raspberry Pi firmware (mailbox property protocol)
- [ ] BCM2835 CPRMAN (clock and power manager)
- [ ] GPIO / pinctrl
- [ ] ARM PL011 UART
- [ ] BCM2835 DMA Engine
- [ ] MMC / SD card controller
- [ ] I2C
- [ ] SPI
- [ ] Display / framebuffer (HDMI)
- [ ] Audio
- [ ] USB controller and HID (keyboard, joystick)
- [ ] Ethernet
- [ ] WiFi
- [ ] Bluetooth

### Kernel Debug Shell

- [x] UART terminal with line discipline (echo, backspace, newline detection)
- [x] Interactive kernel-space debug shell

### Filesystem

- [ ] Virtual filesystem (VFS) layer
- [ ] In-memory filesystem (tmpfs / ramfs)
- [ ] FAT32 (for SD card access)

### Networking

- [ ] IPv4 stack
- [ ] TCP / UDP

### IPC

- [ ] Message passing
- [ ] Shared memory

### User Space & Syscalls

- [ ] EL0 (unprivileged mode) execution
- [ ] Syscall interface
- [ ] Process memory management (user-space page tables)
- [ ] ELF loader (parse and load userspace binaries into process memory)
- [ ] Init process

### SMP (Multi-Core)

- [ ] Secondary core bring-up
- [ ] Per-CPU scheduling (extend current single-core scheduler)

### Self-Test / Kernel Testing

- [ ] In-kernel self-test framework
- [ ] Smoke tests for key subsystems (memory, scheduler, drivers)

### Raspberry Pi 2 & 4 Support

- [ ] Raspberry Pi 2 Model B v1.2 (BCM2837)
- [ ] Raspberry Pi 4 Model B (BCM2711)

### Raspberry Pi 5 Support

- [ ] Raspberry Pi 5 (BCM2712)
