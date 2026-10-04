//! Page-aligned, zero-initialized buffers, as O_DIRECT I/O and DMA
//! transfers require.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::ptr::NonNull;

/// Alignment of every buffer: one page, a multiple of any logical block size.
pub const ALIGNMENT: usize = 4096;

/// A heap buffer aligned on [`ALIGNMENT`], zeroed at creation.
pub struct AlignedBuffer {
    ptr: NonNull<u8>,
    len: usize,
}

// SAFETY: the buffer exclusively owns its allocation, like a `Vec<u8>`.
unsafe impl Send for AlignedBuffer {}

impl AlignedBuffer {
    /// A zeroed buffer of `len` bytes (at least 1).
    pub fn new(len: usize) -> Self {
        let layout = Self::layout(len);
        // SAFETY: `layout` has a non-zero size (at least 1 byte) and a valid
        // power-of-two alignment.
        let ptr = unsafe { alloc_zeroed(layout) };
        let Some(ptr) = NonNull::new(ptr) else {
            std::alloc::handle_alloc_error(layout);
        };
        Self { ptr, len }
    }

    fn layout(len: usize) -> Layout {
        match Layout::from_size_align(len.max(1), ALIGNMENT) {
            Ok(layout) => layout,
            // Only reachable for sizes close to isize::MAX.
            Err(_) => std::alloc::handle_alloc_error(Layout::new::<u8>()),
        }
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Raw pointer, for the kernel.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl std::ops::Deref for AlignedBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // SAFETY: `ptr` points to `len` initialized (zeroed at allocation)
        // bytes owned by this buffer for its whole lifetime.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl std::ops::DerefMut for AlignedBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        // SAFETY: as in `deref`, and `&mut self` guarantees exclusivity.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        // SAFETY: `ptr` was allocated by `alloc_zeroed` with this same layout
        // and is freed exactly once.
        unsafe { dealloc(self.ptr.as_ptr(), Self::layout(self.len)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_are_aligned_zeroed_and_writable() {
        for len in [1, 512, 4096, 1 << 20] {
            let mut buffer = AlignedBuffer::new(len);
            assert_eq!(buffer.as_mut_ptr() as usize % ALIGNMENT, 0);
            assert_eq!(buffer.len(), len);
            assert!(buffer.iter().all(|&byte| byte == 0));
            buffer[len - 1] = 0xab;
            assert_eq!(buffer[len - 1], 0xab);
        }
    }
}
