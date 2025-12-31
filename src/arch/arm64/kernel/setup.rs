use crate::kernel::devicetree::fdt::raw_prop::StandardProperty;
use crate::kernel::devicetree::register_fdt_addr;
use crate::kernel::{devicetree::fdt::Fdt, kernel_addr_size, kernel_bss_size};
use crate::kprintln;
use crate::mm::init_phys_mem;

/// Architecture-specific setup function for ARM64.
///
/// This function parses the Flattened Device Tree (FDT),
/// and initializes the physical memory.
#[unsafe(no_mangle)]
pub fn setup_arch() {
    let fdt_addr = get_fdt_addr();
    let fdt = unsafe { Fdt::new(fdt_addr).unwrap() };

    register_fdt_addr(fdt_addr);
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
        && let Some(node) = fdt.get_node_by_path(stdout_path)
        && let Some(compatible_list) = fdt.parse_standard_prop(&node, StandardProperty::Compatible)
    {
        // validate 'status' property
        if let Some(status) = fdt.parse_standard_prop(&node, StandardProperty::Status)
            && let Some(value) = status.value_as_string().ok()
            && value != "okay"
        {
            return;
        }

        for compatible in compatible_list
            .value_as_string_list_iter()
            .filter_map(|s| s.ok())
        {
            let stdout_compatible_driver = crate::drivers::PLATFORM_DRIVERS
                .iter()
                .find(|d| d.compatible().contains(&compatible));

            if let Some(driver) = stdout_compatible_driver {
                driver.early_init(fdt, stdout_path);
                break;
            }
        }
    }
}

fn parse_memory(fdt: &Fdt) -> Result<([(usize, usize); 32], [(usize, usize); 32]), ()> {
    let memory = fdt.parse_memory();
    let reserved_memory = fdt.parse_reserved_memory()?;

    Ok((memory, reserved_memory))
}
