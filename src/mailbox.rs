use core::ptr::{read_volatile, write_volatile};

// Mailbox memory-mapped registers for BCM2837 (RPi3)
const MAILBOX_BASE: usize = 0x3F00B880;
const MAILBOX_READ: *const u32 = (MAILBOX_BASE + 0x00) as *const u32;
const MAILBOX_STATUS: *const u32 = (MAILBOX_BASE + 0x18) as *const u32;
const MAILBOX_WRITE: *mut u32 = (MAILBOX_BASE + 0x20) as *mut u32;

const MAILBOX_FULL: u32 = 0x80000000;
const MAILBOX_EMPTY: u32 = 0x40000000;

/// A message buffer must be 16-byte aligned.
/// We use a union to ensure this alignment.
#[repr(align(16))]
pub struct MessageBuffer<T>(pub T);

/// Makes a call to the VideoCore IV GPU.
/// The data parameter is a mutable pointer to the message buffer.
pub unsafe fn mailbox_call(channel: u8, data: *mut u32) -> bool {
    // The address sent to the GPU must combine the buffer address with the channel.
    // The GPU will ignore the top 4 bits of the address, so this is safe.
    let addr = (data as u32) | (channel as u32 & 0xF);

    // Wait until the mailbox is not full
    while read_volatile(MAILBOX_STATUS) & MAILBOX_FULL != 0 {}

    // Write the address of our message to the mailbox
    write_volatile(MAILBOX_WRITE, addr);

    loop {
        // Wait until the mailbox is not empty
        while read_volatile(MAILBOX_STATUS) & MAILBOX_EMPTY != 0 {}

        // Read the response and check if it's for our channel
        if read_volatile(MAILBOX_READ) == addr {
            // It is, so check if the response code indicates success
            return read_volatile(data.offset(1)) == 0x80000000;
        }
    }
}