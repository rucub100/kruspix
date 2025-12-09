use alloc::string::String;
use alloc::vec::Vec;

pub trait StandardProperties {
    fn compatible(&self) -> Option<&Vec<String>>;
    fn model(&self) -> Option<&str>;
    fn p_handle(&self) -> Option<u32>;
    fn status(&self) -> Option<&str>;
    fn address_cells(&self) -> u32;
    fn size_cells(&self) -> u32;
    fn reg(&self) -> Option<Vec<(&[u32], &[u32])>>;
    fn virtual_reg(&self) -> Option<u32>;
    fn ranges(&self) -> Option<Vec<(&[u32], &[u32], &[u32])>>;
    fn dma_ranges(&self) -> Option<Vec<(&[u32], &[u32], &[u32])>>;
    fn dma_coherent(&self) -> bool;
    fn dma_noncoherent(&self) -> bool;
}
