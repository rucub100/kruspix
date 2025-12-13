use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::AtomicU32;

use crate::kernel::devicetree::node::Node;
use crate::kernel::sync::{OnceLock, SpinLock};

#[derive(Debug, Clone, Copy)]
pub enum IrqError {
    InvalidConfig,
    ControllerNotFound,
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

static NEXT_VIRQ_BASE: AtomicU32 = AtomicU32::new(0);
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
    // TODO: determine if this is the root controller
    todo!()
}

pub fn register_handler(virq: u32, handler: Box<dyn InterruptHandler>) -> IrqResult<()> {
    todo!()
}

pub fn resolve_virq(node: &Node, index: usize) -> IrqResult<u32> {
    todo!()
}
