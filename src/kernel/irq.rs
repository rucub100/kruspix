use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::kernel::devicetree::interrupts::{InterruptControllerNode, InterruptGeneratingNode};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::sync::{OnceLock, SpinLock};

#[derive(Debug, Clone, Copy)]
pub enum IrqError {
    InvalidConfig,
    InvalidControllerParent,
    TranslationFailed,
    InvalidVirq,
    Busy,
}

pub type IrqResult<T> = Result<T, IrqError>;

pub trait InterruptController: Send + Sync {
    fn enable(&self, hwirq: u32);
    fn disable(&self, hwirq: u32);
    /// Returns the pending interrupt's hardware IRQ number, if any
    fn pending_hwirq(&self) -> Option<u32>;
    fn ack(&self, _hwirq: u32) {}
    /// Translates a specifier to a hardware IRQ number
    fn xlate(&self, specifier: &[u32]) -> IrqResult<u32>;
}

pub trait InterruptHandler: Send + Sync {
    fn handle_irq(&self, virq: u32);
}

// allow closures to act as handlers
impl<F> InterruptHandler for F
where
    F: Fn(u32) + Send + Sync,
{
    fn handle_irq(&self, virq: u32) {
        self(virq);
    }
}

struct IrqDomain {
    node_addr: usize,
    controller: Box<dyn InterruptController>,
    virq_base: u32,
    irq_count: u32,
}

const MAX_IRQS: usize = 256;

static NEXT_VIRQ_BASE: AtomicUsize = AtomicUsize::new(0);
static ROOT_CONTROLLER: OnceLock<IrqDomain> = OnceLock::new();
static CONTROLLERS: SpinLock<Vec<IrqDomain>> = SpinLock::new(Vec::new());
static IRQ_HANDLERS: SpinLock<[Option<Box<dyn InterruptHandler>>; MAX_IRQS]> =
    SpinLock::new([const { None }; MAX_IRQS]);

// TODO: update arch-specific setup to call this function
#[unsafe(no_mangle)]
extern "C" fn _global_irq_dispatch() {
    todo!()
}

pub fn dispatch_irq(virq: u32) {
    todo!()
}

pub fn register_controller(
    node: &Node,
    controller: Box<dyn InterruptController>,
    irq_count: u32,
) -> IrqResult<()> {
    let is_root = match node.interrupt_parent() {
        Some(phandle) => {
            if let Some(own_phandle) = node.phandle() {
                own_phandle.0 == phandle.0
            } else {
                false
            }
        }
        // devicetree parent is assumed to be also the interrupt parent
        None => {
            if let Some(parent) = node.parent() {
                !parent.is_interrupt_controller()
            } else {
                true
            }
        }
    };

    if is_root == ROOT_CONTROLLER.get().is_some() {
        return Err(IrqError::InvalidControllerParent);
    }

    let virq_base = NEXT_VIRQ_BASE.fetch_add(irq_count as usize, Ordering::SeqCst) as u32;
    let irq_domain = IrqDomain {
        node_addr: node as *const _ as usize,
        controller,
        virq_base,
        irq_count,
    };

    if is_root {
        let set_result = ROOT_CONTROLLER.set(irq_domain);
        if set_result.is_err() {
            NEXT_VIRQ_BASE.fetch_sub(irq_count as usize, Ordering::SeqCst);
            return Err(IrqError::InvalidControllerParent);
        }
    } else {
        CONTROLLERS.lock().push(irq_domain);
    }

    Ok(())
}

pub fn register_handler(virq: u32, handler: Box<dyn InterruptHandler>) -> IrqResult<()> {
    todo!()
}

pub fn resolve_virq(node: &Node, index: usize) -> IrqResult<u32> {
    todo!()
}
