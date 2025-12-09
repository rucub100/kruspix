# kruspix

Kruspix is a hands-on, educational kernel for the Raspberry Pi, written in Rust. This project is designed to help you
get a feel for bare metal programming and build your own operating system from the ground up.

## Hardware Support (WIP)

- [ ] Raspberry Pi 2 Model B v1.2 (BCM2837)
- [ ] Raspberry Pi 3 Model B v1.2 (BCM2837)
- [ ] Raspberry Pi 4 Model B (BCM2711)

## Prerequisites

- [Rust](https://www.rust-lang.org/): Make sure you have Rust installed.
  - add the target for Bare ARM64 (see [The rustc book - Platform Support](https://doc.rust-lang.org/rustc/platform-support.html)):
    ```shell
    rustup target add aarch64-unknown-none
    ```
- Install [cargo-binutils](https://github.com/rust-embedded/cargo-binutils#cargo-binutils) for using `llvm-objcopy` and other tools:
    ```shell
    cargo install cargo-binutils
    rustup component add llvm-tools
    ```
- [Raspberry Pi Imager](https://www.raspberrypi.com/software/): To install kruspix OS to a microSD card
- [QEMU](https://www.qemu.org/): Required for emulating the Raspberry Pi and testing the kernel without real hardware

## Getting Started (WIP)

### Project Structure (WIP)

`src/`:
- `arch/`: Architecture-specific code (e.g., ARM64) - boot, memory, CPU, interrupts, MMU, SMP, platform SoCs
- `common/`: Common libraries and utilities
- `docs`: Documentation and design notes
- `drivers/`: Device drivers (e.g., UART, GPIO)
- `fs/`: File system implementations
- `init/`: Kernel initialization code
- `ipc/`: Inter-process communication mechanisms
- `kernel/`: Core kernel components (e.g., scheduler, locking)
- `mm/`: Memory management (e.g., paging, allocators)
- `net/`: Networking stack
- `scripts/`: Scripts for automation
- `tools/`: Tools

TODO: Add instructions on minimal steps to build and run locally (QEMU and hardware)

### Building the Kernel

TODO: User a Dockerfile for a consistent build environment?

#### Build the kernel image
```shell
cargo objcopy --release -- -O binary target/kruspix.img
```
#### Windows: copy to microSD card and eject
```shell
cp .\target\kruspix.img H:\boot\kruspix.img; (New-Object -ComObject Shell.Application).Namespace(17).ParseName("H:").InvokeVerb("Eject")
```


### Qemu (WIP)

TODO: How to start the kernel in QEMU?

### Raspberry Pi `config.txt`

```text
# do not modify this file as it will be overwritten on upgrade.
# create and/or modify usercfg.txt instead.
# https://www.raspberrypi.com/documentation/computers/config_txt.html

#kernel=boot/vmlinuz-rpi
kernel=boot/kruspix.img
#initramfs boot/initramfs-rpi
arm_64bit=1
#include usercfg.txt
enable_uart=1
uart_2ndstage=1
dtparam=watchdog=off
#gpio=22-27=np
enable_jtag_gpio=1
# UM232H        FT232H    JTAG        RPi3 GPIO
# Name  Pin     Name      Func        Pin
# AD0   J2-6    ADBUS0    TCK         25
# AD1   J2-7    ADBUS1    TDI         26
# AD2   J2-8    ADBUS2    TDO         24
# AD3   J2-9    ADBUS3    TMS         27
# AD4   J2-10   ADBUS4    (GPIOL0)    22
# AD5   J2-11   ADBUS5    (GPIOL1)    
# AD6   J2-12   ADBUS6    (GPIOL2)    
# AD7   J2-13   ADBUS7    (GPIOL3)    
# AD0   J1-14   ACBUS0    /TRST       
# AD1   J1-13   ACBUS1    /SRST       
# AD2   J1-12   ACBUS2    (GPIOH2)    
# AD3   J1-11   ACBUS3    (GPIOH3)    
# AD4   J1-10   ACBUS4    (GPIOH4)    
# AD5   J1-9    ACBUS5    (GPIOH5)    
# AD6   J1-8    ACBUS6    (GPIOH6)    
# AD7   J1-7    ACBUS7    (GPIOH7)    
```

### JTAG Debugging

```shell
openocd -f interface/ftdi/um232h.cfg -f board/rpi3.cfg
```

## Roadmap

### Milestones (WIP)

- **v0.0.1 &ndash; Boot & Console**
    - [ ] Basic boot for Raspberry Pi 2/3 (bring up CPU) (P0)
    - [ ] UART console output (P0)
    - [ ] Linker script and minimal kernel binary layout (P0)
    - [ ] Early init logging (P1)

- **v0.0.2 &ndash; Core kernel services**
    - [ ] Exception/interrupt handling (P0)
    - [ ] Timer/tick (P0)
    - [ ] Basic physical memory management (P0)
    - [ ] Simple heap allocator (P1)
    - [ ] Framebuffer or simple display support (P2)

- **v0.0.3 &ndash; Multitasking & drivers**
    - [ ] Preemptive scheduler (P0)
    - [ ] Context switch support (P0)
    - [ ] GPIO and basic peripheral drivers (UART, I2C, SPI) (P1)
    - [ ] SD card / block device driver (P2)

#### 1. Early Boot (Platform-Specific)

- Architecture-specific setup (assembly)
  - Set up CPU mode (e.g., supervisor/kernel mode)
  - MMU and paging setup
  - Basic page table (identity mapping or early mappings)

#### 2. Language Runtime Entry

- Rust entry point (`kernel_main`)
- Set up stack, global state, and runtime environment

#### 3. Platform Discovery

- `setup_arch()` — parse memory map, command line, early I/O
- Parse hardware description (Device Tree, ACPI, UEFI, or custom firmware interface)
- Hardware Abstraction Layer (HAL), e.g. use traits

#### 4. Core Kernel Initialization

- Memory Management
  - Paging
  - Heap allocation
- Interrupts
- Timer and Clock
- Console and Logging

#### 5. Kernel Services

- Scheduler
- Subsystems
  - I/O
  - Networking
  - IPC

#### 6. System Abstractions

- File System (VFS, rootfs)
- Device Drivers (based on hardware description)
- Security (capabilities, isolation, namespaces)

#### 7. Transition to User Space

- Init Process (e.g., `/init`, `systemd`, or custom Rust init)
- Switch to user mode
- Set up syscall interface

#### 8. Optional / Advanced Features

- Async Rust (e.g., for drivers, networking, or task scheduling)
- SMP (Symmetric Multi-Processing)
- Power Management
- Hotplug / Dynamic Device Management
- Kernel Modules / Plugins
- Debugging / Tracing

#### 9. Future Considerations

- User-space environment (shell, GUI, etc.)
- Crash recovery, updates, sandboxing

  
## Learning Material & Resources

### Rust

- [The Rust Programming Language Book](https://doc.rust-lang.org/book/)
- [The Rust Reference](https://doc.rust-lang.org/reference/index.html)
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/index.html)

### Embedded Rust

- [The Discovery book](https://docs.rust-embedded.org/discovery/)
- [The Embedded Rust book](https://docs.rust-embedded.org/book/)
- [The Embedonomicon](https://docs.rust-embedded.org/embedonomicon/)

### Raspberry Pi and ARM

- [Raspberry Pi Documentation](https://www.raspberrypi.com/documentation/)
- [Raspberry Pi Firmware](https://github.com/raspberrypi/firmware)
- [BCM2835 ARM Peripherals](https://datasheets.raspberrypi.com/bcm2835/bcm2835-peripherals.pdf)
- [BCM2836 ARM-local peripherals](https://datasheets.raspberrypi.com/bcm2836/bcm2836-peripherals.pdf)
- [Cortex-A53 MPCore Processor Technical Reference Manual](https://developer.arm.com/documentation/ddi0500/latest/)
- [BCM2711 ARM Peripherals](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf)
- [ARM Cortex-A72](https://en.wikipedia.org/wiki/ARM_Cortex-A72)

### OS Development

- [Writing an OS in Rust (x86_64)](https://os.phil-opp.com/)
- [Simple RPi3 OS in C](https://github.com/s-matyukevich/raspberry-pi-os)
- [Operating System development tutorials in Rust on the Raspberry Pi](https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials)
- [OSDev Wiki](https://wiki.osdev.org/Main_Page)
- [QEMU Documentation](https://wiki.qemu.org/Documentation)
- [Linux Source](https://github.com/torvalds/linux)
- [The Linux Kernel documentation](https://docs.kernel.org/)
- [Device Tree Specification](https://www.devicetree.org/specifications/)
- [Device Bindings](https://github.com/devicetree-org/devicetree-source/tree/master/Bindings)
- [UEFI Specification](https://uefi.org/specifications)
- [POSIX.1-2024](https://pubs.opengroup.org/onlinepubs/9799919799/)

## Contact

TODO: add email
