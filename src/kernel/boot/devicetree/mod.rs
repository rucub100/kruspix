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
        NodeIter::new(token_be_ptr, strings_block_address)
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

    pub fn parse_memory_nodes(&self) -> Result<[(usize, usize); 32], ()> {
        let root = self.root_node()?;
        let mut root_address_cells: Option<u32> = None;
        let mut root_size_cells: Option<u32> = None;

        let aliases = self.aliases_node();
        let reserved_memory = self.reserved_memory_node();
        let chosen = self.chosen_node();
        let cpus = self.cpus_node()?;

        for prop in self.prop_iter(&root) {
            let name = prop.name();
            let standard_prop = name.to_bytes().try_into();

            if standard_prop.is_err() {
                continue;
            }

            let standard_prop = standard_prop?;
            match standard_prop {
                StandardProp::AddressCells => {
                    let value = prop.value_as_u32()?;
                    root_address_cells = Some(value);

                    if root_size_cells.is_some() {
                        break;
                    }
                }
                StandardProp::SizeCells => {
                    let value = prop.value_as_u32()?;
                    root_size_cells = Some(value);
                    
                    if root_address_cells.is_some() {
                        break;
                    }
                }
                _ => { /* ignore other properties */ }
            }
        }

        let root_address_cells = root_address_cells.ok_or(())?;
        let root_size_cells = root_size_cells.ok_or(())?;

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

                        result[memory_reg_index] = (address, size);
                        memory_reg_index += 1;

                        if memory_reg_index >= result.len() {
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

        for entry in self.memory_reservation_block_iter() {
            // TODO subtract from memory ranges
        }

        Ok(result)
    }
}
