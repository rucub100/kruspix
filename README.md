# kruspix

## Introduction

Kruspix is a hands-on, educational kernel for the Raspberry Pi, written in Rust. This project is designed to help you
get a feel for bare metal programming and build your own operating system from the ground up.

## Hardware Support (WIP)

- [ ] Raspberry Pi 2 Model B v1.2 (BCM2837)
- [ ] Raspberry Pi 3 Model B v1.2 (BCM2837)
- [ ] Raspberry Pi 4 Model B (BCM2711)

## Prerequisites

- [Rust](https://www.rust-lang.org/): Make sure you have Rust installed.
- [QEMU](https://www.qemu.org/): Required for emulating the Raspberry Pi and testing the kernel without real hardware.

## Getting Started (WIP)

TODO: Add instructions on minimal steps to build and run locally (QEMU and hardware)

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
  
## Resources

- [The Rust Programming Language Book](https://doc.rust-lang.org/book/)
- [The Rust Reference](https://doc.rust-lang.org/reference/index.html)
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/index.html)


- [The Discovery book](https://docs.rust-embedded.org/discovery/)
- [The Embedded Rust book](https://docs.rust-embedded.org/book/)
- [The Embedonomicon](https://docs.rust-embedded.org/embedonomicon/)


- [Raspberry Pi Documentation](https://www.raspberrypi.com/documentation/)
- [BCM2836 ARM-local peripherals](https://datasheets.raspberrypi.com/bcm2836/bcm2836-peripherals.pdf)
- [Cortex-A53 MPCore Processor Technical Reference Manual](https://developer.arm.com/documentation/ddi0500/latest/)
- [BCM2711 ARM Peripherals](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf)
- [ARM Cortex-A72](https://en.wikipedia.org/wiki/ARM_Cortex-A72)


- [Operating System development tutorials in Rust on the Raspberry Pi](https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials)
- [OSDev Wiki](https://wiki.osdev.org/Main_Page)
- [QEMU Documentation](https://wiki.qemu.org/Documentation)

## Contact

TODO: add email

## License

TODO