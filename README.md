# kruspix

Kruspix is a hands-on, educational bare-metal OS kernel for the Raspberry Pi, written in Rust.
I built this project to learn OS fundamentals from the ground up &ndash; boot, memory management,
exceptions, scheduling, and device drivers, all without an OS underneath.

![kruspix demo](docs/kruspix_demo_2026_05_11.gif)

## Hardware Support

- [ ] Raspberry Pi 2 Model B v1.2 (BCM2837)
- [X] Raspberry Pi 3 Model B v1.2 (BCM2837)
- [ ] Raspberry Pi 4 Model B (BCM2711)
- [ ] Raspberry Pi 5 (BCM2712)

## Prerequisites

- [Rust](https://www.rust-lang.org/): Make sure you have Rust installed.
  - Add the target for Bare ARM64 (see [The rustc book - Platform Support](https://doc.rust-lang.org/rustc/platform-support.html)):
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

## Getting Started

### Building the Kernel

#### Build the kernel image
```shell
cargo objcopy --release -- -O binary target/kruspix.img
```

### Run in QEMU

Before running, the Raspberry Pi 3 device tree binary must be placed at `raspberrypi/bcm2710-rpi-3-b.dtb`.
This file is not included in the repository &ndash; download it from the
[official Raspberry Pi firmware repo](https://github.com/raspberrypi/firmware/tree/master/boot):

```shell
# create the folder and download the DTB
mkdir raspberrypi
curl -L -o raspberrypi/bcm2710-rpi-3-b.dtb https://github.com/raspberrypi/firmware/raw/master/boot/bcm2710-rpi-3-b.dtb
```

Then launch the kernel in QEMU:

```shell
cargo run
```

This uses the runner configured in `.cargo/config.toml` &ndash; it launches `qemu-system-aarch64`
with the `raspi3b` machine, the BCM2710 device tree, and serial output on stdio.

### Run on Hardware

#### Copy to microSD card and eject (Windows)
```shell
cp .\target\kruspix.img H:\boot\kruspix.img; (New-Object -ComObject Shell.Application).Namespace(17).ParseName("H:").InvokeVerb("Eject")
```

#### `usercfg.txt`

```text
kernel=boot/kruspix.img
arm_64bit=1
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

## JTAG Debugging

```shell
openocd -f interface/ftdi/um232h.cfg -f board/rpi3.cfg
```

## Project Structure

`src/`:
- `arch/` &ndash; architecture-specific code (ARM64 boot, MMU, CPU, exception vectors)
- `common/` &ndash; general utilities and data structures
- `drivers/` &ndash; platform device drivers (DTB-based model)
- `fs/` &ndash; filesystem (planned)
- `init/` &ndash; init system (planned)
- `ipc/` &ndash; inter-process communication (planned)
- `kernel/` &ndash; core kernel services (scheduler, IRQ, sync, shell, logging)
- `mm/` &ndash; physical memory management and heap
- `net/` &ndash; networking stack (planned)

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full list of completed milestones and planned features.

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

📧 [info@ruslan-curbanov.de](mailto:info@ruslan-curbanov.de)

*Feel free to reach out regarding bug reports, technical discussions, or collaboration opportunities.*