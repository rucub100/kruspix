// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>
//
// @vibe-coded

use alloc::string::String;
use alloc::sync::Arc;
use core::hint::spin_loop;
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::arch::cpu::{clean_data_cache_range, memory_barrier};
use crate::drivers::syscon::get_rpi_firmware;
use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::irq::{InterruptHandler, enable_irq, register_handler, resolve_virq};
use crate::kernel::sync::SpinLock;
use crate::kprintln;
use crate::mm::{alloc_page, map_io_region, virt_to_phys};

use super::{Dma2DTransfer, DmaEngine, DmaError, register_dma_engine};

const BCM2835_DMA_CHAN_SIZE: usize = 0x100;
const BCM2835_DMA_CS_OFFSET: usize = 0x00;
const BCM2835_DMA_ADDR_OFFSET: usize = 0x04;
const BCM2835_DMA_DEBUG_OFFSET: usize = 0x20;
const BCM2835_DMA_ENABLE_OFFSET: usize = 0xFF0;

const BCM2835_DMA_CS_ACTIVE: u32 = 1 << 0;
const BCM2835_DMA_CS_END: u32 = 1 << 1;
const BCM2835_DMA_CS_INT: u32 = 1 << 2;
const BCM2835_DMA_CS_ERR: u32 = 1 << 8;
const BCM2835_DMA_CS_PRIORITY_SHIFT: u32 = 16;
const BCM2835_DMA_CS_PANIC_PRIORITY_SHIFT: u32 = 20;
const BCM2835_DMA_CS_WAIT_FOR_WRITES: u32 = 1 << 28;
const BCM2835_DMA_CS_DIS_DEBUG: u32 = 1 << 29;
const BCM2835_DMA_CS_ABORT: u32 = 1 << 30;
const BCM2835_DMA_CS_RESET: u32 = 1 << 31;

const BCM2835_DMA_TI_INT_EN: u32 = 1 << 0;
const BCM2835_DMA_TI_TDMODE: u32 = 1 << 1;
const BCM2835_DMA_TI_D_INC: u32 = 1 << 4;
const BCM2835_DMA_TI_D_WIDTH: u32 = 1 << 5;
const BCM2835_DMA_TI_S_INC: u32 = 1 << 8;
const BCM2835_DMA_TI_S_WIDTH: u32 = 1 << 9;

const BCM2835_DMA_DEBUG_LAST_NOT_SET_ERR: u32 = 1 << 0;
const BCM2835_DMA_DEBUG_FIFO_ERR: u32 = 1 << 1;
const BCM2835_DMA_DEBUG_READ_ERR: u32 = 1 << 2;
const BCM2835_DMA_DEBUG_CLEAR_MASK: u32 = BCM2835_DMA_DEBUG_LAST_NOT_SET_ERR
    | BCM2835_DMA_DEBUG_FIFO_ERR
    | BCM2835_DMA_DEBUG_READ_ERR;

const DMA_BUS_ALIAS_MASK: u32 = 0xC000_0000;
const DMA_BUS_ALIAS_NONALLOCATING: u32 = 0x8000_0000;
const DMA_PREFERRED_CHANNELS: &[u8] = &[0, 2, 4, 6];
const DMA_RESET_POLL_LIMIT: usize = 1_000_000;
const DMA_WAIT_POLL_LIMIT: usize = 128_000_000;
const DMA_CS_DEFAULTS: u32 = (8 << BCM2835_DMA_CS_PRIORITY_SHIFT)
    | (8 << BCM2835_DMA_CS_PANIC_PRIORITY_SHIFT)
    | BCM2835_DMA_CS_WAIT_FOR_WRITES
    | BCM2835_DMA_CS_DIS_DEBUG;

const DMA_ERROR_NONE: u32 = 0;
const DMA_ERROR_TIMEOUT: u32 = 1;
const DMA_ERROR_FAULT: u32 = 2;
const DMA_ERROR_INVALID: u32 = 3;
const DMA_ERROR_BUSY: u32 = 4;

static DMA_TRANSFERS_STARTED: AtomicU32 = AtomicU32::new(0);
static DMA_IRQ_COMPLETIONS: AtomicU32 = AtomicU32::new(0);
static DMA_POLL_COMPLETIONS: AtomicU32 = AtomicU32::new(0);
static DMA_TIMEOUTS: AtomicU32 = AtomicU32::new(0);
static DMA_FAULTS: AtomicU32 = AtomicU32::new(0);
static DMA_BUSY_ERRORS: AtomicU32 = AtomicU32::new(0);
static DMA_INVALID_ERRORS: AtomicU32 = AtomicU32::new(0);
static DMA_LAST_CS: AtomicU32 = AtomicU32::new(0);
static DMA_LAST_DEBUG: AtomicU32 = AtomicU32::new(0);
static DMA_LAST_ERROR: AtomicU32 = AtomicU32::new(DMA_ERROR_NONE);

#[derive(Clone, Copy)]
pub struct Bcm2835DmaStats {
    pub transfers_started: u32,
    pub irq_completions: u32,
    pub poll_completions: u32,
    pub timeouts: u32,
    pub faults: u32,
    pub busy_errors: u32,
    pub invalid_errors: u32,
    pub last_cs: u32,
    pub last_debug: u32,
    pub last_error: u32,
}

pub fn get_stats() -> Bcm2835DmaStats {
    Bcm2835DmaStats {
        transfers_started: DMA_TRANSFERS_STARTED.load(Ordering::Relaxed),
        irq_completions: DMA_IRQ_COMPLETIONS.load(Ordering::Relaxed),
        poll_completions: DMA_POLL_COMPLETIONS.load(Ordering::Relaxed),
        timeouts: DMA_TIMEOUTS.load(Ordering::Relaxed),
        faults: DMA_FAULTS.load(Ordering::Relaxed),
        busy_errors: DMA_BUSY_ERRORS.load(Ordering::Relaxed),
        invalid_errors: DMA_INVALID_ERRORS.load(Ordering::Relaxed),
        last_cs: DMA_LAST_CS.load(Ordering::Relaxed),
        last_debug: DMA_LAST_DEBUG.load(Ordering::Relaxed),
        last_error: DMA_LAST_ERROR.load(Ordering::Relaxed),
    }
}

pub fn reset_stats() {
    DMA_TRANSFERS_STARTED.store(0, Ordering::Relaxed);
    DMA_IRQ_COMPLETIONS.store(0, Ordering::Relaxed);
    DMA_POLL_COMPLETIONS.store(0, Ordering::Relaxed);
    DMA_TIMEOUTS.store(0, Ordering::Relaxed);
    DMA_FAULTS.store(0, Ordering::Relaxed);
    DMA_BUSY_ERRORS.store(0, Ordering::Relaxed);
    DMA_INVALID_ERRORS.store(0, Ordering::Relaxed);
    DMA_LAST_CS.store(0, Ordering::Relaxed);
    DMA_LAST_DEBUG.store(0, Ordering::Relaxed);
    DMA_LAST_ERROR.store(DMA_ERROR_NONE, Ordering::Relaxed);
}

#[repr(C, align(32))]
struct DmaControlBlock {
    info: u32,
    src: u32,
    dst: u32,
    length: u32,
    stride: u32,
    next: u32,
    pad: [u32; 2],
}

impl DmaControlBlock {
    const fn new() -> Self {
        Self {
            info: 0,
            src: 0,
            dst: 0,
            length: 0,
            stride: 0,
            next: 0,
            pad: [0; 2],
        }
    }
}

struct Bcm2835Dma {
    id: String,
    reg_base: usize,
    channel: u8,
    virq: u32,
    control_block_va: usize,
    control_block_bus: u32,
    transfer_lock: SpinLock<()>,
    irq_complete: AtomicBool,
    irq_error: AtomicBool,
}

impl Bcm2835Dma {
    const fn burst_length(value: u32) -> u32 {
        (value & 0xF) << 12
    }

    const fn dma_bus_addr(addr: u32) -> u32 {
        (addr & !DMA_BUS_ALIAS_MASK) | DMA_BUS_ALIAS_NONALLOCATING
    }

    fn new(
        id: String,
        reg_base: usize,
        channel: u8,
        virq: u32,
        control_block_va: usize,
        control_block_bus: u32,
    ) -> Self {
        Self {
            id,
            reg_base,
            channel,
            virq,
            control_block_va,
            control_block_bus,
            transfer_lock: SpinLock::new(()),
            irq_complete: AtomicBool::new(false),
            irq_error: AtomicBool::new(false),
        }
    }

    fn channel_base(&self) -> usize {
        self.reg_base + (self.channel as usize * BCM2835_DMA_CHAN_SIZE)
    }

    fn read_shared_reg(&self, offset: usize) -> u32 {
        let reg = self.reg_base + offset;
        let reg_ptr = reg as *const u32;
        // SAFETY: `reg` points at a mapped DMA MMIO register inside the controller region.
        unsafe { core::ptr::read_volatile(reg_ptr) }
    }

    fn write_shared_reg(&self, offset: usize, value: u32) {
        let reg = self.reg_base + offset;
        let reg_ptr = reg as *mut u32;
        // SAFETY: `reg` points at a mapped DMA MMIO register inside the controller region.
        unsafe { core::ptr::write_volatile(reg_ptr, value) };
    }

    fn read_channel_reg(&self, offset: usize) -> u32 {
        let reg = self.channel_base() + offset;
        let reg_ptr = reg as *const u32;
        // SAFETY: `reg` points at a mapped DMA channel MMIO register.
        unsafe { core::ptr::read_volatile(reg_ptr) }
    }

    fn write_channel_reg(&self, offset: usize, value: u32) {
        let reg = self.channel_base() + offset;
        let reg_ptr = reg as *mut u32;
        // SAFETY: `reg` points at a mapped DMA channel MMIO register.
        unsafe { core::ptr::write_volatile(reg_ptr, value) };
    }

    fn enable_channel(&self) {
        let enabled = self.read_shared_reg(BCM2835_DMA_ENABLE_OFFSET);
        self.write_shared_reg(
            BCM2835_DMA_ENABLE_OFFSET,
            enabled | (1 << u32::from(self.channel)),
        );
    }

    fn clear_channel_state(&self) {
        self.write_channel_reg(
            BCM2835_DMA_CS_OFFSET,
            BCM2835_DMA_CS_END | BCM2835_DMA_CS_INT | BCM2835_DMA_CS_ERR,
        );
        self.write_channel_reg(BCM2835_DMA_DEBUG_OFFSET, BCM2835_DMA_DEBUG_CLEAR_MASK);
    }

    fn reset_channel(&self) -> Result<(), DmaError> {
        self.write_channel_reg(BCM2835_DMA_CS_OFFSET, BCM2835_DMA_CS_RESET);

        for _ in 0..DMA_RESET_POLL_LIMIT {
            if (self.read_channel_reg(BCM2835_DMA_CS_OFFSET) & BCM2835_DMA_CS_RESET) == 0 {
                self.clear_channel_state();
                return Ok(());
            }
            spin_loop();
        }

        Err(DmaError::Timeout)
    }

    fn abort_channel(&self) {
        self.write_channel_reg(BCM2835_DMA_CS_OFFSET, BCM2835_DMA_CS_ABORT);
        let _ = self.reset_channel();
    }

    fn control_block_ptr(&self) -> *mut DmaControlBlock {
        self.control_block_va as *mut DmaControlBlock
    }

    fn fill_control_block(&self, transfer: Dma2DTransfer) -> Result<(), DmaError> {
        if transfer.row_len_bytes == 0
            || transfer.row_count == 0
            || transfer.row_len_bytes > u16::MAX as u32
            || transfer.row_count > u16::MAX as u32
            || transfer.src_stride > u16::MAX as u32
            || transfer.dst_stride > u16::MAX as u32
        {
            DMA_INVALID_ERRORS.fetch_add(1, Ordering::Relaxed);
            DMA_LAST_ERROR.store(DMA_ERROR_INVALID, Ordering::Relaxed);
            return Err(DmaError::InvalidTransfer);
        }

        let length = transfer.row_len_bytes | (transfer.row_count << 16);
        let stride = transfer.src_stride | (transfer.dst_stride << 16);
        let burst_size = if self.channel == 0 { 8 } else { 2 };

        // SAFETY: `control_block_va` comes from a dedicated page allocation owned by this driver.
        // `transfer_lock` serializes submissions so no concurrent mutable access occurs here.
        unsafe {
            let cb = &mut *self.control_block_ptr();
            *cb = DmaControlBlock {
                info: BCM2835_DMA_TI_INT_EN
                    | BCM2835_DMA_TI_TDMODE
                    | BCM2835_DMA_TI_D_INC
                    | BCM2835_DMA_TI_D_WIDTH
                    | BCM2835_DMA_TI_S_INC
                    | BCM2835_DMA_TI_S_WIDTH
                    | Self::burst_length(burst_size),
                src: Self::dma_bus_addr(transfer.src_bus_addr),
                dst: Self::dma_bus_addr(transfer.dst_bus_addr),
                length,
                stride,
                next: 0,
                pad: [0; 2],
            };
        }

        // SAFETY: The DMA engine reads the control block from RAM via the bus alias, so the cache
        // line containing the control block must be cleaned before the transfer starts.
        unsafe {
            clean_data_cache_range(self.control_block_va, size_of::<DmaControlBlock>());
        }

        Ok(())
    }

    fn start_transfer(&self) -> Result<(), DmaError> {
        self.reset_channel()?;
        self.irq_complete.store(false, Ordering::Release);
        self.irq_error.store(false, Ordering::Release);

        DMA_TRANSFERS_STARTED.fetch_add(1, Ordering::Relaxed);
        DMA_LAST_ERROR.store(DMA_ERROR_NONE, Ordering::Relaxed);
        self.write_channel_reg(BCM2835_DMA_ADDR_OFFSET, self.control_block_bus);
        self.write_channel_reg(BCM2835_DMA_CS_OFFSET, DMA_CS_DEFAULTS | BCM2835_DMA_CS_ACTIVE);
        Ok(())
    }

    fn poll_transfer_result(&self) -> Option<Result<(), DmaError>> {
        let cs = self.read_channel_reg(BCM2835_DMA_CS_OFFSET);
        let debug = self.read_channel_reg(BCM2835_DMA_DEBUG_OFFSET);
        DMA_LAST_CS.store(cs, Ordering::Relaxed);
        DMA_LAST_DEBUG.store(debug, Ordering::Relaxed);

        if (cs & BCM2835_DMA_CS_ERR) != 0 || (debug & BCM2835_DMA_DEBUG_CLEAR_MASK) != 0 {
            DMA_FAULTS.fetch_add(1, Ordering::Relaxed);
            DMA_LAST_ERROR.store(DMA_ERROR_FAULT, Ordering::Relaxed);
            self.clear_channel_state();
            return Some(Err(DmaError::Fault));
        }

        if (cs & (BCM2835_DMA_CS_END | BCM2835_DMA_CS_INT)) != 0 {
            DMA_POLL_COMPLETIONS.fetch_add(1, Ordering::Relaxed);
            self.clear_channel_state();
            return Some(Ok(()));
        }

        None
    }

    fn wait_for_transfer(&self) -> Result<(), DmaError> {
        for _ in 0..DMA_WAIT_POLL_LIMIT {
            if self.irq_error.swap(false, Ordering::AcqRel) {
                DMA_FAULTS.fetch_add(1, Ordering::Relaxed);
                DMA_LAST_ERROR.store(DMA_ERROR_FAULT, Ordering::Relaxed);
                return Err(DmaError::Fault);
            }
            if self.irq_complete.swap(false, Ordering::AcqRel) {
                return Ok(());
            }
            if let Some(result) = self.poll_transfer_result() {
                return result;
            }

            spin_loop();
        }

        DMA_TIMEOUTS.fetch_add(1, Ordering::Relaxed);
        DMA_LAST_ERROR.store(DMA_ERROR_TIMEOUT, Ordering::Relaxed);
        Err(DmaError::Timeout)
    }

    fn select_channel(mask: u32) -> Option<u8> {
        DMA_PREFERRED_CHANNELS
            .iter()
            .copied()
            .find(|channel| (mask & (1 << u32::from(*channel))) != 0)
    }

    fn phys_to_dma_bus(pa: usize) -> Result<u32, DmaError> {
        let pa = u32::try_from(pa).map_err(|_| DmaError::InvalidTransfer)?;
        Ok(Self::dma_bus_addr(pa))
    }
}

impl Device for Bcm2835Dma {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        self.enable_channel();
        register_handler(self.virq, self.clone()).map_err(|_| DriverInitError::Retry)?;
        enable_irq(self.virq).map_err(|_| DriverInitError::Retry)?;
        register_dma_engine(self.clone() as Arc<dyn DmaEngine>)
            .map_err(|_| DriverInitError::DeviceFailed)?;
        kprintln!("[bcm2835-dma] using channel {}", self.channel);
        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl InterruptHandler for Bcm2835Dma {
    fn handle_irq(&self, _virq: u32) {
        let cs = self.read_channel_reg(BCM2835_DMA_CS_OFFSET);
        let debug = self.read_channel_reg(BCM2835_DMA_DEBUG_OFFSET);
        DMA_LAST_CS.store(cs, Ordering::Relaxed);
        DMA_LAST_DEBUG.store(debug, Ordering::Relaxed);

        if (cs & (BCM2835_DMA_CS_END | BCM2835_DMA_CS_INT | BCM2835_DMA_CS_ERR)) == 0
            && (debug & BCM2835_DMA_DEBUG_CLEAR_MASK) == 0
        {
            return;
        }

        if (cs & BCM2835_DMA_CS_ERR) != 0 || (debug & BCM2835_DMA_DEBUG_CLEAR_MASK) != 0 {
            self.irq_error.store(true, Ordering::Release);
            DMA_LAST_ERROR.store(DMA_ERROR_FAULT, Ordering::Relaxed);
        } else {
            self.irq_complete.store(true, Ordering::Release);
            DMA_IRQ_COMPLETIONS.fetch_add(1, Ordering::Relaxed);
        }

        self.clear_channel_state();
    }
}

impl DmaEngine for Bcm2835Dma {
    fn copy_2d(&self, transfer: Dma2DTransfer) -> Result<(), DmaError> {
        let _guard = self.transfer_lock.try_lock().ok_or_else(|| {
            DMA_BUSY_ERRORS.fetch_add(1, Ordering::Relaxed);
            DMA_LAST_ERROR.store(DMA_ERROR_BUSY, Ordering::Relaxed);
            DmaError::Busy
        })?;

        self.fill_control_block(transfer)?;
        self.start_transfer()?;

        match self.wait_for_transfer() {
            Ok(()) => {
                memory_barrier();
                Ok(())
            }
            Err(err) => {
                self.abort_channel();
                Err(err)
            }
        }
    }
}

pub struct Bcm2835DmaDriver {
    dev_registry: DriverRegistry<Bcm2835Dma>,
}

impl Bcm2835DmaDriver {
    pub const fn new() -> Self {
        Self {
            dev_registry: DriverRegistry::new(),
        }
    }
}

impl PlatformDriver for Bcm2835DmaDriver {
    fn compatible(&self) -> &[&str] {
        &["brcm,bcm2835-dma"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        let reg = node.reg().ok_or(DriverInitError::DeviceTreeError)?;
        if reg.len() != 1 {
            return Err(DriverInitError::DeviceTreeError);
        }

        let firmware = get_rpi_firmware().ok_or(DriverInitError::Retry)?;
        let usable_mask = firmware
            .get_dma_channels()
            .map_err(|_| DriverInitError::DeviceFailed)?;
        let channel = Bcm2835Dma::select_channel(usable_mask).ok_or_else(|| {
            kprintln!("[bcm2835-dma] [WARNING] no usable full DMA channel found");
            DriverInitError::DeviceFailed
        })?;
        let virq = resolve_virq(node, channel as usize).map_err(|_| DriverInitError::Retry)?;

        let (phys_addr, length) = node
            .resolve_phys_address_and_length(0)
            .ok_or(DriverInitError::DeviceTreeError)?;
        let reg_base = map_io_region(phys_addr, length);

        let control_block_page = alloc_page();
        if control_block_page.is_null() {
            return Err(DriverInitError::DeviceFailed);
        }

        let control_block_va = control_block_page as usize;
        let control_block_bus = Bcm2835Dma::phys_to_dma_bus(virt_to_phys(control_block_va))
            .map_err(|_| DriverInitError::DeviceFailed)?;

        // SAFETY: `control_block_va` points at a full page allocated exclusively for the DMA
        // control block, so writing the initial zeroed value is within the allocation.
        unsafe {
            core::ptr::write(control_block_va as *mut DmaControlBlock, DmaControlBlock::new());
        }

        let dev = Arc::new(Bcm2835Dma::new(
            node.path(),
            reg_base,
            channel,
            virq,
            control_block_va,
            control_block_bus,
        ));

        dev.clone().global_setup(node)?;
        self.dev_registry.add_device(node.path(), dev);
        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: Bcm2835DmaDriver = Bcm2835DmaDriver::new();
