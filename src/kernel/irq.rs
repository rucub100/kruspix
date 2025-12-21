use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::kernel::devicetree::get_devicetree;
use crate::kernel::devicetree::interrupts::{
    InterruptControllerNode, InterruptControllerOrNexusNode, InterruptGeneratingNode,
    InterruptNexusNode, InterruptSpecifier,
};
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
    fn set_virq_base(&self, _virq_base: u32) -> IrqResult<()> { Ok(()) }
    fn enable(&self, hwirq: u32);
    fn disable(&self, hwirq: u32);
    /// Returns the pending interrupt's hardware IRQ number, if any
    fn pending_hwirq(&self) -> Option<u32>;
    fn ack(&self, _hwirq: u32) {}
    /// Translates a specifier to a hardware IRQ number
    fn xlate(&self, specifier: &InterruptSpecifier) -> IrqResult<u32>;
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
    controller: Arc<dyn InterruptController>,
    virq_base: u32,
    irq_count: u32,
}

const MAX_IRQS: usize = 256;

static NEXT_VIRQ_BASE: AtomicUsize = AtomicUsize::new(0);
static ROOT_CONTROLLER: OnceLock<IrqDomain> = OnceLock::new();
static CONTROLLERS: SpinLock<Vec<IrqDomain>> = SpinLock::new(Vec::new());
static IRQ_HANDLERS: SpinLock<[Option<Arc<dyn InterruptHandler>>; MAX_IRQS]> =
    SpinLock::new([const { None }; MAX_IRQS]);

#[unsafe(no_mangle)]
pub extern "C" fn global_irq_dispatch() {
    if let Some(root) = ROOT_CONTROLLER.get() {
        while let Some(hwirq) = root.controller.pending_hwirq() {
            dispatch_irq(root.virq_base + hwirq);
        }
    }
}

pub fn dispatch_irq(virq: u32) {
    if virq as usize >= MAX_IRQS {
        return;
    }

    let handler = {
        let handlers = IRQ_HANDLERS.lock();
        handlers[virq as usize].clone()
    };

    if let Some(handler) = handler {
        handler.handle_irq(virq);
    }
}

pub fn register_controller(
    node: &Node,
    controller: Arc<dyn InterruptController>,
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

    if NEXT_VIRQ_BASE.load(Ordering::Relaxed) >= (MAX_IRQS - irq_count as usize) {
        return Err(IrqError::InvalidConfig);
    }

    let virq_base = NEXT_VIRQ_BASE.fetch_add(irq_count as usize, Ordering::SeqCst) as u32;

    controller.set_virq_base(virq_base)?;

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

pub fn register_handler(virq: u32, handler: Arc<dyn InterruptHandler>) -> IrqResult<()> {
    if virq as usize >= MAX_IRQS {
        return Err(IrqError::InvalidVirq);
    }

    let mut handlers = IRQ_HANDLERS.lock();
    if handlers[virq as usize].is_some() {
        return Err(IrqError::Busy);
    }

    handlers[virq as usize] = Some(handler);

    let domains = CONTROLLERS.lock();
    let domain = ROOT_CONTROLLER
        .get()
        .into_iter()
        .chain(domains.iter())
        .find(|d| virq >= d.virq_base && virq < d.virq_base + d.irq_count)
        .ok_or(IrqError::InvalidVirq)?;

    let hwirq = virq - domain.virq_base;
    domain.controller.enable(hwirq);

    Ok(())
}

pub fn resolve_virq(node: &Node, index: usize) -> IrqResult<u32> {
    if node.interrupts_extended().is_some() {
        // TODO: extended interrupts have higher precedence than regular interrupts
        // and they are mutually exclusive to regular interrupts
        todo!()
    }

    let interrupts = node.interrupts().ok_or(IrqError::InvalidConfig)?;

    let dt = get_devicetree().ok_or(IrqError::InvalidConfig)?;
    let int_parent_phandle = node.interrupt_parent();
    let int_parent = if let Some(phandle) = int_parent_phandle {
        dt.node_by_phandle(phandle).ok_or(IrqError::InvalidConfig)?
    } else {
        node.parent().ok_or(IrqError::InvalidControllerParent)?
    };

    if !int_parent.is_interrupt_controller() {
        return Err(IrqError::InvalidControllerParent);
    }

    if int_parent.interrupt_map().is_some() {
        // FIXME: handle interrupt nexus nodes
        todo!()
    }

    let interrupt_cells = int_parent
        .interrupt_cells()
        .ok_or(IrqError::InvalidConfig)?;
    let specifier = interrupts
        .iter(interrupt_cells)
        .nth(index)
        .ok_or(IrqError::InvalidConfig)?;

    let node_addr = int_parent as *const _ as usize;
    let domains = CONTROLLERS.lock();
    let domain = ROOT_CONTROLLER
        .get()
        .into_iter()
        .chain(domains.iter())
        .find(|d| d.node_addr == node_addr)
        .ok_or(IrqError::InvalidControllerParent)?;

    let hwirq = domain.controller.xlate(&specifier)?;
    Ok(domain.virq_base + hwirq)
}
