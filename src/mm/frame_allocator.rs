use core::mem::size_of;
use core::ptr;

pub unsafe trait PageFrameAllocator {
    /// Allocate a page frame and return its starting address.
    ///
    /// Returns `Some(address)` if allocation is successful, or `None` if no frames are available.
    unsafe fn alloc_frame(&mut self) -> *mut u8;

    /// Deallocate a page frame given its starting address.
    ///
    /// # Arguments
    /// * `addr` - The starting address of the page frame to be deallocated.
    unsafe fn dealloc_frame(&mut self, ptr: *mut u8);
}

pub struct BitMapFrameAllocator {
    frame_size: usize,
    total_frames: usize,
    bitmap_frames: usize,
    start: *mut usize,
}

impl BitMapFrameAllocator {
    /// Create a new `BitMapFrameAllocator`.
    ///
    /// # Arguments
    /// * `start` - The starting address of the memory region to manage.
    /// * `size` - The size of the memory region to manage (in bytes). Minimum size is 10 times the frame size.
    /// * `frame_size` - The size of each page frame (in bytes). Must be a power of two.
    pub fn new(start: usize, size: usize, frame_size: usize) -> Self {
        assert!(frame_size.is_power_of_two());
        assert!(frame_size.is_multiple_of(size_of::<usize>()));
        assert!(size > 10 * frame_size);

        let start_aligned = start.next_multiple_of(frame_size);
        let end_aligned = (start + size).next_multiple_of(frame_size);
        let size_aligned = if end_aligned > start + size {
            end_aligned - frame_size
        } else {
            end_aligned
        } - start_aligned;

        let total_frames = size_aligned / frame_size;
        let start = start_aligned as *mut usize;

        let bitmap_size = (total_frames + 7) / 8;
        let bitmap_frames = (bitmap_size + frame_size - 1) / frame_size;

        unsafe {
            for i in 0..(bitmap_frames * frame_size / size_of::<usize>()) {
                start.add(i).write_volatile(0);
            }

            let usize_to_mask = bitmap_frames / (size_of::<usize>() * 8);
            let remainder_bits = bitmap_frames % (size_of::<usize>() * 8);

            if usize_to_mask > 0 {
                for i in 0..usize_to_mask {
                    start.add(i).write_volatile(usize::MAX);
                }
            }

            if remainder_bits > 0 {
                start.add(usize_to_mask).write_volatile(
                    1usize
                        .checked_shl(remainder_bits as u32)
                        .map(|x| x - 1)
                        .unwrap_or(usize::MAX),
                );
            }
        }

        BitMapFrameAllocator {
            frame_size,
            total_frames,
            bitmap_frames,
            start,
        }
    }
}

unsafe impl PageFrameAllocator for BitMapFrameAllocator {
    unsafe fn alloc_frame(&mut self) -> *mut u8 {
        let mut index = 0;
        let bitmap_size = self.bitmap_frames * self.frame_size / size_of::<usize>();

        loop {
            if index >= bitmap_size {
                break;
            }

            let chunk = unsafe { *self.start.add(index) };

            if chunk == usize::MAX {
                index += 1;
                continue;
            }
            let chunk_index = chunk.trailing_ones();

            let frame_index = index * size_of::<usize>() * 8 + chunk_index as usize;
            if frame_index >= self.total_frames {
                break;
            }

            let byte_offset = frame_index * self.frame_size;
            let pointer = unsafe { (self.start as *mut u8).add(byte_offset) };

            unsafe {
                *self.start.add(index) |= 1 << chunk_index;
            }

            return pointer;
        }

        ptr::null_mut()
    }

    unsafe fn dealloc_frame(&mut self, ptr: *mut u8) {
        let frame_index = (ptr as usize - self.start as usize) / self.frame_size;
        let index = frame_index / (size_of::<usize>() * 8);
        let chunk_index = frame_index % (size_of::<usize>() * 8);

        unsafe {
            *self.start.add(index) &= !(1 << chunk_index);
        }
    }
}
