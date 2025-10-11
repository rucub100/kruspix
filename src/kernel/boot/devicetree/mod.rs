use fdt_header::{FdtHeader, FdtHeaderPtrExt};
use fdt_reserve_entry::FdtReserveEntry;
use fdt_reserve_entry::FdtReserveEntryIter;
use fdt_structure_block::StructureBlockIter;

use node::{Node, NodeIter};
use prop::{PropIter, StandardProp};

mod fdt_header;
mod fdt_prop;
mod fdt_reserve_entry;
mod fdt_structure_block;

pub mod node;
pub mod prop;

pub struct Fdt {
    address: usize,
}

impl Fdt {
    pub fn new(address: usize) -> Result<Self, ()> {
        let header = FdtHeader::at_addr(address);
        if header.is_valid() {
            Ok(Fdt { address })
        } else {
            Err(())
        }
    }

    pub fn memory_reservation_block_iter(&self) -> FdtReserveEntryIter {
        let header = FdtHeader::at_addr(self.address);
        let offset = header.mem_rsv_map_offset() as usize;
        let address = self.address + offset;
        let fdt_reserve_entry_ptr = address as *const FdtReserveEntry;
        fdt_reserve_entry_ptr.into()
    }

    pub fn structure_block_iter(&self) -> StructureBlockIter {
        let header = FdtHeader::at_addr(self.address);
        let structure_block_offset = header.structure_block_offset() as usize;
        let strings_block_offset = header.strings_block_offset() as usize;
        let structure_block_address = self.address + structure_block_offset;
        let strings_block_address = self.address + strings_block_offset;
        let token_be_ptr = structure_block_address as *const u32;
        StructureBlockIter::new(token_be_ptr, strings_block_address)
    }

    pub fn node_iter(&self) -> NodeIter {
        let header = FdtHeader::at_addr(self.address);
        let structure_block_offset = header.structure_block_offset() as usize;
        let strings_block_offset = header.strings_block_offset() as usize;
        let structure_block_address = self.address + structure_block_offset;
        let strings_block_address = self.address + strings_block_offset;
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
        let header = FdtHeader::at_addr(self.address);
        let strings_block_offset = header.strings_block_offset() as usize;
        let strings_block_address = self.address + strings_block_offset;
        PropIter::new(node.props_ptr(), strings_block_address)
    }

    pub fn child_iter(&self, node: &Node) -> NodeIter {
        let header = FdtHeader::at_addr(self.address);
        let strings_block_offset = header.strings_block_offset() as usize;
        let strings_block_address = self.address + strings_block_offset;
        NodeIter::new(node.children_ptr(), strings_block_address, 1)
    }

    pub fn address_and_size_cells(&self, node: &Node) -> Result<(u32, u32), ()> {
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
        let (root_address_cells, root_size_cells) = self.address_and_size_cells(&root)?;

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
        let header = FdtHeader::at_addr(self.address);
        result[index] = (self.address, header.total_size() as usize);
        index += 1;

        let reserved_memory_node = self.reserved_memory_node();
        if let Some(reserved_memory_node) = reserved_memory_node {
            let root = self.root_node()?;
            let (root_address_cells, root_size_cells) = self.address_and_size_cells(&root)?;
            let (address_cells, size_cells) = self.address_and_size_cells(&reserved_memory_node)?;

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
