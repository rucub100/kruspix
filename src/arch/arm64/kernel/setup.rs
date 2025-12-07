use crate::kernel::devicetree::fdt::prop::StandardProp;
use crate::kernel::{
    devicetree::{fdt::Fdt, set_fdt},
    kernel_addr_size, kernel_bss_size,
};
use crate::kprintln;
use crate::mm::init_phys_mem;

/// Architecture-specific setup function for ARM64.
///
/// This function parses the Flattened Device Tree (FDT),
/// and initializes the physical memory.
#[unsafe(no_mangle)]
pub fn setup_arch() {
    let fdt_addr = get_fdt_addr();
    let fdt = Fdt::new(fdt_addr).unwrap();

    parse_parameters(&fdt);

    kprintln!("FDT address: {:#x}", fdt_addr);

    let (kernel_addr, kernel_size) = kernel_addr_size();
    let kernel_bss_size = kernel_bss_size();
    kprintln!(
        "Kernel address: {:#x}, size: {:#x} bytes, BSS size: {:#x} bytes",
        kernel_addr,
        kernel_size,
        kernel_bss_size
    );

    kprintln!("Parsing Flattened Device Tree (FDT)...");
    let (mem, reserved_mem) = parse_memory(&fdt).unwrap();

    set_fdt(fdt);

    kprintln!("Initializing physical memory...");
    init_phys_mem(mem, reserved_mem, (kernel_addr, kernel_size), fdt_addr);
}

#[inline(always)]
fn get_fdt_addr() -> usize {
    let fdt_addr: usize;

    unsafe {
        core::arch::asm!(
        "ldr {tmp}, =__fdt_address",
        "ldr {out}, [{tmp}]",
        tmp = out(reg) _,
        out = out(reg) fdt_addr);
    }

    fdt_addr
}

fn parse_parameters(fdt: &Fdt) {
    let (_bootargs, stdout_path, _stdin_path) = fdt.parse_chosen();

    if let Some(stdout_path) = stdout_path
        && !stdout_path.is_empty()
        && let Some(node) = if stdout_path.contains('/') {
            fdt.get_node_by_path(stdout_path)
        } else {
            fdt.get_node_by_alias(stdout_path)
        }
        && let Some(compatible) = fdt.prop_iter(&node).find(|prop| {
            prop.name()
                .to_bytes()
                .try_into()
                .is_ok_and(|prop: StandardProp| prop == StandardProp::Compatible)
        })
    {
        for compatible in compatible
            .value_as_string_list_iter()
            .filter_map(|s| s.ok())
            .filter_map(|s| s.to_str().ok())
        {
            let stdout_compatible_driver = crate::drivers::PLATFORM_DRIVERS
                .iter()
                .find(|d| d.compatible() == compatible);

            if let Some(driver) = stdout_compatible_driver {
                driver.static_init(fdt, &node);
                break;
            }
        }
    }
}

fn parse_memory(fdt: &Fdt) -> Result<([(usize, usize); 32], [(usize, usize); 32]), ()> {
    let memory = fdt.parse_memory()?;
    let reserved_memory = fdt.parse_reserved_memory()?;

    Ok((memory, reserved_memory))
}
