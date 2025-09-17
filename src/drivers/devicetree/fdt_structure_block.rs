pub const FDT_BEGIN_NODE: u32 = const { u32::to_be(0x00000001) };
pub const FDT_END_NODE: u32 = const { u32::to_be(0x00000002) };
pub const FDT_PROP: u32 = const { u32::to_be(0x00000003) };
pub const FDT_NOP: u32 = const { u32::to_be(0x00000004) };
pub const FDT_END: u32 = const { u32::to_be(0x00000009) };
