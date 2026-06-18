use core::alloc::{GlobalAlloc, Layout};
use core::fmt;
use core::ptr::NonNull;

use crate::MiMalloc;

/// Error returned when a safe Rust allocation request cannot be satisfied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocError {
    layout: Layout,
}

impl AllocError {
    #[inline]
    pub const fn layout(self) -> Layout {
        self.layout
    }
}

impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "mimalloc allocation failed for size {} with alignment {}",
            self.layout.size(),
            self.layout.align()
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AllocError {}

/// Error returned when size/alignment parameters do not form a valid layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutError {
    size: usize,
    align: usize,
}

impl LayoutError {
    #[inline]
    pub const fn size(self) -> usize {
        self.size
    }

    #[inline]
    pub const fn align(self) -> usize {
        self.align
    }
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid allocation layout for size {} with alignment {}",
            self.size, self.align
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LayoutError {}

/// Error returned by size/alignment convenience allocation constructors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryAllocError {
    Layout(LayoutError),
    Alloc(AllocError),
}

impl fmt::Display for TryAllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(err) => err.fmt(f),
            Self::Alloc(err) => err.fmt(f),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TryAllocError {}

/// Owned byte allocation backed by [`MiMalloc`].
///
/// This is the crate's minimal safe allocation wrapper. It converts C-style
/// null allocation failure into [`AllocError`] and releases the allocation on
/// drop. It intentionally does not expose heap handles, options, statistics, or
/// allocator-owned metadata.
pub struct Allocation {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl Allocation {
    /// Allocates an uninitialized byte block for `layout`.
    #[inline]
    pub fn new(layout: Layout) -> Result<Self, AllocError> {
        Self::allocate(layout, false)
    }

    /// Allocates a zero-initialized byte block for `layout`.
    #[inline]
    pub fn zeroed(layout: Layout) -> Result<Self, AllocError> {
        Self::allocate(layout, true)
    }

    /// Builds a layout from `size` and `align`, then allocates an uninitialized
    /// byte block.
    #[inline]
    pub fn with_size_align(size: usize, align: usize) -> Result<Self, TryAllocError> {
        let layout = Layout::from_size_align(size, align)
            .map_err(|_| TryAllocError::Layout(LayoutError { size, align }))?;
        Self::new(layout).map_err(TryAllocError::Alloc)
    }

    /// Builds a layout from `size` and `align`, then allocates a zeroed byte
    /// block.
    #[inline]
    pub fn zeroed_with_size_align(size: usize, align: usize) -> Result<Self, TryAllocError> {
        let layout = Layout::from_size_align(size, align)
            .map_err(|_| TryAllocError::Layout(LayoutError { size, align }))?;
        Self::zeroed(layout).map_err(TryAllocError::Alloc)
    }

    #[inline]
    fn allocate(layout: Layout, zeroed: bool) -> Result<Self, AllocError> {
        if layout.size() == 0 {
            return Ok(Self {
                ptr: dangling_for(layout),
                layout,
            });
        }

        let ptr = unsafe {
            if zeroed {
                MiMalloc.alloc_zeroed(layout)
            } else {
                MiMalloc.alloc(layout)
            }
        };

        let ptr = NonNull::new(ptr).ok_or(AllocError { layout })?;
        Ok(Self { ptr, layout })
    }

    #[inline]
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.layout.size()
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.layout.size() == 0
    }

    #[inline]
    pub const fn as_non_null(&self) -> NonNull<u8> {
        self.ptr
    }

    #[inline]
    pub const fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for Allocation {
    fn drop(&mut self) {
        if self.layout.size() != 0 {
            unsafe { MiMalloc.dealloc(self.ptr.as_ptr(), self.layout) };
        }
    }
}

impl fmt::Debug for Allocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Allocation")
            .field("ptr", &self.ptr)
            .field("layout", &self.layout)
            .finish()
    }
}

#[inline]
fn dangling_for(layout: Layout) -> NonNull<u8> {
    unsafe { NonNull::new_unchecked(layout.align() as *mut u8) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_allocation_round_trips_bytes() {
        let layout = Layout::from_size_align(64, 16).expect("layout");
        let allocation = Allocation::new(layout).expect("allocation");
        assert_eq!(allocation.layout(), layout);
        assert_eq!(allocation.len(), 64);
        assert_eq!(allocation.as_ptr() as usize % 16, 0);
    }

    #[test]
    fn zeroed_allocation_returns_zeroed_bytes() {
        let allocation = Allocation::zeroed_with_size_align(32, 8).expect("allocation");
        let bytes = unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
        assert!(bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn zero_sized_allocation_does_not_call_allocator() {
        let layout = Layout::from_size_align(0, 64).expect("layout");
        let allocation = Allocation::new(layout).expect("allocation");
        assert!(allocation.is_empty());
        assert_eq!(allocation.as_ptr() as usize % 64, 0);
    }

    #[test]
    fn invalid_layout_reports_rust_error() {
        let err = Allocation::with_size_align(8, 3).expect_err("invalid alignment");
        match err {
            TryAllocError::Layout(err) => {
                assert_eq!(err.size(), 8);
                assert_eq!(err.align(), 3);
            }
            TryAllocError::Alloc(_) => panic!("expected layout error"),
        }
    }
}
