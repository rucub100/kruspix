use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::iter;
use core::ptr::NonNull;

use super::prop::{Property, PropertyValue, StandardProperty};
use super::standard_properties::StandardProperties;

pub struct Node {
    name: String,
    properties: Vec<Property>,
    children: Vec<Box<Node>>,
    parent: Option<NonNull<Node>>,
}

impl Node {
    pub const fn new(name: String, parent: Option<NonNull<Node>>) -> Self {
        Node {
            name,
            properties: Vec::new(),
            children: Vec::new(),
            parent,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn node_name(&self) -> &str {
        &self.name.split('@').next().unwrap_or(&self.name)
    }

    pub fn unit_address(&self) -> Option<&str> {
        let parts: Vec<&str> = self.name.split('@').collect();
        if parts.len() > 1 {
            Some(parts[1])
        } else {
            None
        }
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    pub fn is_compatible_with(&self, compatible: &str) -> bool {
        self.compatible()
            .is_some_and(|compatible_list| compatible_list.iter().any(|c| c == compatible))
    }

    pub fn path(&self) -> String {
        let mut segments = Vec::with_capacity(16);
        let mut current = Some(self);

        while let Some(node) = current {
            if !node.name.is_empty() {
                segments.push(node.name.as_str());
            }
            current = node.parent();
        }

        if segments.is_empty() {
            return String::from("/");
        }

        let path_len = segments.iter().map(|s| s.len() + 1).sum();
        let mut path = String::with_capacity(path_len);

        segments.iter().rev().for_each(|name| {
            path.push('/');
            path.push_str(name);
        });

        path
    }

    pub fn properties(&self) -> &Vec<Property> {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut Vec<Property> {
        &mut self.properties
    }

    pub fn children(&self) -> &Vec<Box<Node>> {
        &self.children
    }

    pub fn children_mut(&mut self) -> &mut Vec<Box<Node>> {
        &mut self.children
    }

    pub fn parent(&self) -> Option<&Node> {
        self.parent.map(|p| unsafe { p.as_ref() })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        let mut stack = Vec::new();
        stack.push(self);

        iter::from_fn(move || {
            let node = stack.pop()?;
            stack.extend(node.children.iter().rev().map(|b| b.as_ref()));
            Some(node)
        })
    }
}

impl StandardProperties for Node {
    fn compatible(&self) -> Option<&Vec<String>> {
        self.properties
            .iter()
            .find(|p| p.name() == StandardProperty::COMPATIBLE)
            .and_then(|p| match &p.value() {
                PropertyValue::Standard(StandardProperty::Compatible(compatible_list)) => {
                    Some(compatible_list)
                }
                _ => None,
            })
    }

    fn model(&self) -> Option<&str> {
        self.properties
            .iter()
            .find(|p| p.name() == StandardProperty::MODEL)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::Model(model)) => Some(model.as_str()),
                _ => None,
            })
    }

    fn p_handle(&self) -> Option<u32> {
        self.properties
            .iter()
            .find(|p| p.name() == StandardProperty::P_HANDLE)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::PHandle(p_handle)) => Some(*p_handle),
                _ => None,
            })
    }

    fn status(&self) -> Option<&str> {
        self.properties
            .iter()
            .find(|p| p.name() == StandardProperty::STATUS)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::Status(status)) => Some(status.as_str()),
                _ => None,
            })
    }

    fn address_cells(&self) -> u32 {
        self.properties
            .iter()
            .find(|p| p.name() == StandardProperty::ADDRESS_CELLS)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::AddressCells(addr_cells)) => {
                    Some(*addr_cells)
                }
                _ => None,
            })
            .unwrap_or(2)
    }

    fn size_cells(&self) -> u32 {
        self.properties
            .iter()
            .find(|p| p.name() == StandardProperty::SIZE_CELLS)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::SizeCells(size_cells)) => {
                    Some(*size_cells)
                }
                _ => None,
            })
            .unwrap_or(1)
    }

    fn reg(&self) -> Option<Vec<(&[u32], &[u32])>> {
        todo!()
    }

    fn virtual_reg(&self) -> Option<u32> {
        todo!()
    }

    fn ranges(&self) -> Option<Vec<(&[u32], &[u32], &[u32])>> {
        todo!()
    }

    fn dma_ranges(&self) -> Option<Vec<(&[u32], &[u32], &[u32])>> {
        todo!()
    }

    fn dma_coherent(&self) -> bool {
        self.properties
            .iter()
            .any(|p| p.name() == StandardProperty::DMA_COHERENT)
    }

    fn dma_noncoherent(&self) -> bool {
        self.properties
            .iter()
            .any(|p| p.name() == StandardProperty::DMA_NONCOHERENT)
    }
}
