use fdt_header::{FdtHeader, FdtHeaderPtrExt};
use fdt_reserve_entry::FdtReserveEntry;
use fdt_reserve_entry::FdtReserveEntryIter;
use fdt_structure_block::StructureBlockIter;
use node::{Node, NodeIter};
use prop::{Prop, PropIter, StandardProp};

pub mod fdt_header;
pub mod fdt_prop;
pub mod fdt_reserve_entry;
pub mod fdt_structure_block;

pub mod node;
pub mod prop;

pub struct Fdt {
    fdt_header: *const FdtHeader,
}

// SAFETY: Fdt is supposed to be read-only and used early in the boot process,
// so at that time there is no SMP or other concurrency (e.g., interrupts) to worry about.
unsafe impl Send for Fdt {}
unsafe impl Sync for Fdt {}

impl Fdt {
    pub fn new(address: usize) -> Result<Self, ()> {
        let fdt_header = FdtHeader::at_addr(address);
        if fdt_header.is_valid() {
            Ok(Fdt { fdt_header })
        } else {
            Err(())
        }
    }

    pub fn address(&self) -> usize {
        self.fdt_header.addr()
    }

    pub fn size(&self) -> u32 {
        self.fdt_header.total_size()
    }

    pub fn version(&self) -> u32 {
        self.fdt_header.version()
    }

    pub fn last_compatible_version(&self) -> u32 {
        self.fdt_header.last_comp_version()
    }

    pub fn boot_cpuid_phys(&self) -> u32 {
        self.fdt_header.boot_cpuid_phys()
    }

    pub fn memory_reservation_block_iter(&self) -> FdtReserveEntryIter {
        let header = FdtHeader::at_addr(self.fdt_header.addr());
        let offset = header.mem_rsv_map_offset() as usize;
        let address = self.fdt_header.addr() + offset;
        let fdt_reserve_entry_ptr = address as *const FdtReserveEntry;
        fdt_reserve_entry_ptr.into()
    }

    pub fn structure_block_iter(&self) -> StructureBlockIter {
        let header = FdtHeader::at_addr(self.fdt_header.addr());
        let structure_block_offset = header.structure_block_offset() as usize;
        let strings_block_offset = header.strings_block_offset() as usize;
        let structure_block_address = self.fdt_header.addr() + structure_block_offset;
        let strings_block_address = self.fdt_header.addr() + strings_block_offset;
        let token_be_ptr = structure_block_address as *const u32;
        StructureBlockIter::new(token_be_ptr, strings_block_address)
    }

    pub fn node_iter(&self) -> NodeIter {
        let header = FdtHeader::at_addr(self.fdt_header.addr());
        let structure_block_offset = header.structure_block_offset() as usize;
        let strings_block_offset = header.strings_block_offset() as usize;
        let structure_block_address = self.fdt_header.addr() + structure_block_offset;
        let strings_block_address = self.fdt_header.addr() + strings_block_offset;
        let token_be_ptr = structure_block_address as *const u32;
        NodeIter::new(token_be_ptr, strings_block_address, isize::MAX)
    }

    pub fn root_node(&self) -> Result<Node, ()> {
        let root = self.node_iter().find(|node| node.is_root());
        root.ok_or(())
    }

    pub fn aliases_node(&self) -> Option<Node> {
        self.node_iter().find(|node| node.is_aliases())
    }

    pub fn memory_node_iter(&self) -> impl Iterator<Item = Node> {
        self.node_iter().filter(|node| node.is_memory())
    }

    pub fn reserved_memory_node(&self) -> Option<Node> {
        self.node_iter().find(|node| node.is_reserved_memory())
    }

    pub fn chosen_node(&self) -> Option<Node> {
        self.node_iter().find(|node| node.is_chosen())
    }

    pub fn cpus_node(&self) -> Result<Node, ()> {
        let root = self.node_iter().find(|node| node.is_cpus());
        root.ok_or(())
    }

    pub fn prop_iter(&self, node: &Node) -> PropIter {
        let header = FdtHeader::at_addr(self.fdt_header.addr());
        let strings_block_offset = header.strings_block_offset() as usize;
        let strings_block_address = self.fdt_header.addr() + strings_block_offset;
        PropIter::new(node.props_ptr(), strings_block_address)
    }

    pub fn child_iter(&self, node: &Node) -> NodeIter {
        let header = FdtHeader::at_addr(self.fdt_header.addr());
        let strings_block_offset = header.strings_block_offset() as usize;
        let strings_block_address = self.fdt_header.addr() + strings_block_offset;
        NodeIter::new(node.children_ptr(), strings_block_address, 1)
    }

    pub fn get_node_by_alias(&self, alias: &str) -> Option<Node> {
        self.prop_iter(&self.aliases_node()?)
            .find(|prop| prop.name().to_str().ok() == Some(alias))
            .and_then(|prop| {
                let path = prop.value_as_string().ok()?.to_str().ok()?;
                self.get_node_by_path(path)
            })
    }

    pub fn get_node_by_path(&self, path: &str) -> Option<Node> {
        let mut current_node = self.root_node().ok()?;
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            let next_node = self
                .child_iter(&current_node)
                .find(|node| node.name().to_str().ok() == Some(segment));
            if let Some(next_node) = next_node {
                current_node = next_node;
            } else {
                return None;
            }
        }

        Some(current_node)
    }

    pub fn parse_chosen(&self) -> (Option<&str>, Option<&str>, Option<&str>) {
        let mut bootargs: Option<&str> = None;
        let mut stdout_path: Option<&str> = None;
        let mut stdin_path: Option<&str> = None;

        fn extract_and_set_path(prop: &Prop, dest: &mut Option<&str>) {
            if let Some(path) = prop.value_as_string().ok()
                && let Some(path) = path.to_str().ok()
            {
                let path = match path.split_once(':') {
                    Some((p, _)) => p,
                    None => path,
                };

                dest.replace(path);
            }
        }

        if let Some(chosen_node) = self.chosen_node() {
            for prop in self.prop_iter(&chosen_node) {
                match prop.name().to_bytes() {
                    b"bootargs" => {
                        if let Some(value) = prop.value_as_string().ok()
                            && let Some(value) = value.to_str().ok()
                        {
                            bootargs.replace(value);
                        }
                    }
                    // ends_with to support legacy names
                    x if x.ends_with(b"stdout-path") => {
                        extract_and_set_path(&prop, &mut stdout_path)
                    }
                    b"stdin-path" => extract_and_set_path(&prop, &mut stdin_path),
                    _ => continue,
                }
            }
        }

        (bootargs, stdout_path, stdin_path)
    }

    pub fn parse_address_and_size_cells(&self, node: &Node) -> Result<(u32, u32), ()> {
        let mut address_cells: Option<u32> = None;
        let mut size_cells: Option<u32> = None;

        for prop in self.prop_iter(&node) {
            let name = prop.name();
            let standard_prop = name.to_bytes().try_into();

            // consider only standard properties
            if standard_prop.is_err() {
                continue;
            }

            let standard_prop = standard_prop?;
            match standard_prop {
                StandardProp::AddressCells => {
                    let value = prop.value_as_u32()?;
                    address_cells = Some(value);

                    if size_cells.is_some() {
                        break;
                    }
                }
                StandardProp::SizeCells => {
                    let value = prop.value_as_u32()?;
                    size_cells = Some(value);

                    if address_cells.is_some() {
                        break;
                    }
                }
                _ => { /* ignore other properties */ }
            }
        }

        let address_cells = address_cells.ok_or(())?;
        let size_cells = size_cells.ok_or(())?;
        Ok((address_cells, size_cells))
    }

    pub fn parse_memory(&self) -> Result<[(usize, usize); 32], ()> {
        let root = self.root_node()?;
        let (root_address_cells, root_size_cells) = self.parse_address_and_size_cells(&root)?;

        if root_address_cells == 0 || root_size_cells == 0 {
            return Err(());
        }

        let mut result: [(usize, usize); 32] = [(0, 0); 32];
        let mut memory_reg_index: usize = 0;
        for prop in self
            .memory_node_iter()
            .flat_map(|node| self.prop_iter(&node))
        {
            let name = prop.name();
            let standard_prop = name.to_bytes().try_into();

            if standard_prop.is_err() {
                continue;
            }

            let standard_prop = standard_prop?;
            match standard_prop {
                StandardProp::Reg => {
                    for (address, size) in prop.value_as_prop_encoded_array_cells_pair_iter(
                        root_address_cells,
                        root_size_cells,
                    ) {
                        result[memory_reg_index] = (address, size);
                        memory_reg_index += 1;

                        if memory_reg_index >= result.len() {
                            // TODO flag overflow to signal missing memory regions
                            break;
                        }
                    }

                    // we only care about the 'reg' property
                    break;
                }
                _ => { /* ignore other properties */ }
            }
        }

        Ok(result)
    }

    pub fn parse_reserved_memory(&self) -> Result<[(usize, usize); 32], ()> {
        let mut result: [(usize, usize); 32] = [(0, 0); 32];
        let mut index: usize = 0;

        // add FDT blob itself to reserved memory
        let header = FdtHeader::at_addr(self.fdt_header.addr());
        result[index] = (self.fdt_header.addr(), header.total_size() as usize);
        index += 1;

        let reserved_memory_node = self.reserved_memory_node();
        if let Some(reserved_memory_node) = reserved_memory_node {
            let root = self.root_node()?;
            let (root_address_cells, root_size_cells) = self.parse_address_and_size_cells(&root)?;
            let (address_cells, size_cells) =
                self.parse_address_and_size_cells(&reserved_memory_node)?;

            // address translation not supported
            if address_cells != root_address_cells || size_cells != root_size_cells {
                return Err(());
            }
            for prop in self.prop_iter(&reserved_memory_node) {
                if prop.value().is_empty() {
                    continue;
                }

                let name = prop.name();
                let standard_prop = name.to_bytes().try_into();

                if standard_prop.is_err() {
                    continue;
                }

                let standard_prop = standard_prop?;
                match standard_prop {
                    StandardProp::Ranges => {
                        // address translation not supported
                        unimplemented!()
                    }
                    _ => { /* ignore other properties */ }
                }
            }

            let mut size_dynamic = 0;
            for child_prop in self
                .child_iter(&reserved_memory_node)
                .flat_map(|node| self.prop_iter(&node))
            {
                if child_prop.value().is_empty() {
                    continue;
                }

                let name = child_prop.name().to_bytes();
                match name {
                    val if val == Into::<&[u8]>::into(StandardProp::Reg) => {
                        for (address, size) in child_prop
                            .value_as_prop_encoded_array_cells_pair_iter(address_cells, size_cells)
                        {
                            if index >= result.len() {
                                return Err(());
                            }

                            result[index] = (address, size);
                            index += 1;
                        }
                    }
                    b"size" => {
                        for size in child_prop.value_as_prop_encoded_array_cells_iter(size_cells) {
                            size_dynamic += size;
                        }
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }

        for entry in self.memory_reservation_block_iter() {
            if index >= result.len() {
                return Err(());
            }

            let address = usize::try_from(entry.address()).map_err(|_| ())?;
            let size = usize::try_from(entry.size()).map_err(|_| ())?;

            result[index] = (address, size);
            index += 1;
        }

        Ok(result)
    }
}
