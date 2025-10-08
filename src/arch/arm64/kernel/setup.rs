use crate::kernel::boot::devicetree::Fdt;

pub fn setup_arch() {
    parse_fdt();
}

pub fn parse_fdt() {
    let fdt_addr: usize;

    unsafe {
        core::arch::asm!("mov {}, x0", out(reg) fdt_addr);
    }

    let fdt = Fdt::new(fdt_addr);
    let fdt = fdt.unwrap();

    let root = fdt.root_node().expect("FDT: Failed to get root node");
    let mut root_address_cells: Option<u32> = None;
    let mut root_size_cells: Option<u32> = None;

    let aliases = fdt.aliases_node();
    let reserved_memory = fdt.reserved_memory_node();
    let chosen = fdt.chosen_node();
    let cpus = fdt.cpus_node().expect("FDT: Failed to get CPUS node");

    for prop in fdt.prop_iter(&root) {
        let name = prop.name();

        match name.to_bytes() {
            b"#address-cells" => {
                let value = prop.value_as_u32().unwrap();
                root_address_cells = Some(value);
            }
            b"#size-cells" => {
                let value = prop.value_as_u32().unwrap();
                root_size_cells = Some(value);
            }
            b"model" => {
                let value = prop.value_as_string().unwrap();
                // TODO use value
            }
            b"compatible" => {
                for compatible in prop.value_as_string_list_iter() {
                    let compatible = compatible.unwrap();
                    // TODO use compatible
                }
            }
            b"serial-number" => {
                let value = prop.value_as_string().unwrap();
                // TODO use value
            }
            b"chassis-type" => {
                let value = prop.value_as_string().unwrap();
                // TODO use value
            }
            _ => { /* ignore other properties */ }
        }
    }

    let root_address_cells = root_address_cells.expect("FDT: Missing #address-cells in root node");
    let root_size_cells = root_size_cells.expect("FDT: Missing #size-cells in root node");

    let mut memory_reg: [(usize, usize); 32] = [(0, 0); 32];
    let mut memory_reg_index: usize = 0;
    for prop in fdt.memory_node_iter().flat_map(|node| fdt.prop_iter(&node)) {
        let name = prop.name();

        match name.to_bytes() {
            b"reg" => {
                let value = prop.value();
                let chunk_size = (root_address_cells + root_size_cells) as usize * 4;
                assert_eq!(value.len() % chunk_size, 0);

                for chunk in value.chunks(chunk_size) {
                    let mut address: usize = 0;
                    for addr_chunk in chunk[..root_address_cells as usize * 4].chunks(4) {
                        address <<= 32;
                        address += u32::from_be_bytes(addr_chunk.try_into().unwrap()) as usize;
                    }

                    let mut size: usize = 0;
                    for size_chunk in chunk[root_address_cells as usize * 4..].chunks(4) {
                        size <<= 32;
                        size += u32::from_be_bytes(size_chunk.try_into().unwrap()) as usize;
                    }

                    memory_reg[memory_reg_index] = (address, size);
                    memory_reg_index += 1;

                    if memory_reg_index >= memory_reg.len() {
                        break;
                    }
                }
            }
            _ => { /* ignore other properties */ }
        }
    }

    if let Some(reserved_memory_node) = reserved_memory {
        // TODO subtract reserved memory ranges from array
    }

    for entry in fdt.memory_reservation_block_iter() {
        // TODO subtract from memory ranges
    }
}
