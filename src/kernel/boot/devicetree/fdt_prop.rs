#[repr(C, align(4))]
pub struct FdtProp
{
    len: u32,
    nameoff: u32,
}

impl FdtProp {
    pub fn value_len(&self) -> u32 {
        u32::from_be(self.len)
    }

    pub fn name_offset(&self) -> u32 {
        u32::from_be(self.nameoff)
    }
}