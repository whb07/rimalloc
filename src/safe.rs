use core::alloc::{GlobalAlloc, Layout};
use core::fmt;
use core::ptr::NonNull;

use crate::MiMalloc;

/// Error returned when a safe Rust allocation request cannot be satisfied.
///
/// Cause: mimalloc returned null for a valid Rust [`Layout`].
///
/// Recovery: keep or drop existing allocations, reduce the requested size or
/// alignment, or propagate the error. The failed request does not create an
/// allocation that needs to be freed.
///
/// Allocation failure is platform and overcommit dependent, so examples should
/// treat this as the error branch from safe allocation APIs rather than trying
/// to force a reliable runtime failure:
///
/// ```rust,no_run
/// use core::alloc::Layout;
/// use mechanical_mimalloc::{AllocError, Allocation};
///
/// fn inspect(err: AllocError) {
///     assert!(err.layout().align().is_power_of_two());
/// }
///
/// let layout = Layout::from_size_align(1024, 16).unwrap();
/// if let Err(err) = Allocation::new(layout) {
///     inspect(err);
/// }
/// ```
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
///
/// Cause: a size/alignment convenience constructor received parameters rejected
/// by [`Layout::from_size_align`].
///
/// Recovery: pass a non-zero power-of-two alignment and a size/alignment pair
/// that fits Rust's layout rules.
///
/// ```rust
/// use mechanical_mimalloc::{Allocation, TryAllocError};
///
/// let err = Allocation::with_size_align(8, 3).unwrap_err();
/// match err {
///     TryAllocError::Layout(layout) => {
///         assert_eq!(layout.size(), 8);
///         assert_eq!(layout.align(), 3);
///     }
///     TryAllocError::Alloc(_) => unreachable!(),
/// }
/// ```
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
///
/// `TryAllocError::Layout` means no allocator call was made because the
/// requested layout was invalid. `TryAllocError::Alloc` means the layout was
/// valid but allocation failed.
///
/// ```rust
/// use mechanical_mimalloc::{Allocation, TryAllocError};
///
/// let err = Allocation::zeroed_with_size_align(1, 3).unwrap_err();
/// assert!(matches!(err, TryAllocError::Layout(_)));
/// ```
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
///
/// `Allocation` currently keeps a single move-only type with raw pointer access
/// only. [`Allocation::new`] returns uninitialized memory; safe code can query
/// the layout and raw pointer, but it cannot read bytes through a safe slice
/// API. Callers that use the raw pointer must initialize memory before reading
/// it.
///
/// [`Allocation::zeroed`] returns memory initialized to zero, but this type
/// still does not expose safe byte-slice access. Future slice APIs will be
/// added only with an initialization model that cannot expose uninitialized
/// bytes through safe code.
///
/// Zero-sized allocations do not call the allocator. They store an aligned
/// sentinel pointer and drop is a no-op.
///
/// `Allocation` is move-only. Dropping it frees the allocation exactly once;
/// there is no `Clone`, `Copy`, `from_raw`, `into_raw`, or resize API. Allocate
/// a new block and copy through raw pointers if resizing is needed before a
/// safe resize API is designed.
///
/// ```rust
/// use core::alloc::Layout;
/// use mechanical_mimalloc::Allocation;
///
/// let layout = Layout::from_size_align(32, 8).unwrap();
/// let allocation = Allocation::zeroed(layout).unwrap();
/// assert_eq!(allocation.len(), 32);
/// assert!(!allocation.is_empty());
/// ```
///
/// Uninitialized allocations do not expose safe byte-slice reads:
///
/// ```rust,compile_fail
/// use core::alloc::Layout;
/// use mechanical_mimalloc::Allocation;
///
/// let layout = Layout::from_size_align(16, 8).unwrap();
/// let allocation = Allocation::new(layout).unwrap();
/// let _bytes = allocation.as_bytes();
/// ```
///
/// `Allocation` is not `Copy`:
///
/// ```rust,compile_fail
/// use core::alloc::Layout;
/// use mechanical_mimalloc::Allocation;
///
/// let layout = Layout::from_size_align(16, 8).unwrap();
/// let allocation = Allocation::new(layout).unwrap();
/// let moved = allocation;
/// let _use_after_move = allocation.len();
/// drop(moved);
/// ```
pub struct Allocation {
    ptr: NonNull<u8>,
    layout: Layout,
}

// SAFETY[AUDIT:owned-allocation]: `Allocation` has unique ownership of a block
// allocated by mimalloc. Mimalloc supports freeing allocations from a different
// thread, and safe methods do not expose references into the block.
unsafe impl Send for Allocation {}

// SAFETY[AUDIT:owned-allocation]: shared references to `Allocation` expose only
// metadata and raw pointers. Safe code cannot read or mutate allocation
// contents through `&Allocation`; any raw pointer dereference remains an unsafe
// caller boundary.
unsafe impl Sync for Allocation {}

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

    /// Returns the layout used to allocate this block.
    #[inline]
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    /// Returns the requested allocation size in bytes.
    ///
    /// This is metadata only; it does not prove that the allocation contents
    /// are initialized.
    #[inline]
    pub const fn len(&self) -> usize {
        self.layout.size()
    }

    /// Returns whether this is a zero-sized allocation.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.layout.size() == 0
    }

    /// Returns the allocation pointer as [`NonNull<u8>`].
    ///
    /// The pointer is valid only while this `Allocation` is alive. Callers must
    /// not free it manually, must not create aliasing references that violate
    /// Rust's aliasing rules, and must not read uninitialized memory. For
    /// zero-sized allocations this is an aligned sentinel pointer and must not
    /// be dereferenced.
    #[inline]
    pub const fn as_non_null(&self) -> NonNull<u8> {
        self.ptr
    }

    /// Returns the allocation pointer.
    ///
    /// The pointer is valid only while this `Allocation` is alive. Callers must
    /// not free it manually, must not create aliasing references that violate
    /// Rust's aliasing rules, and must not read uninitialized memory. For
    /// zero-sized allocations this is an aligned sentinel pointer and must not
    /// be dereferenced.
    #[inline]
    pub const fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for Allocation {
    fn drop(&mut self) {
        if self.layout.size() != 0 {
            // SAFETY[AUDIT:owned-allocation]: non-zero `Allocation` values are
            // constructed only from successful `MiMalloc` allocation calls and
            // are freed exactly once from `Drop`.
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
    // SAFETY[AUDIT:owned-allocation]: `Layout` alignments are non-zero powers
    // of two, so the integer value is an aligned non-null sentinel for
    // zero-sized allocations and is never dereferenced by safe code.
    unsafe { NonNull::new_unchecked(layout.align() as *mut u8) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;

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
    fn uninitialized_allocation_can_be_written_before_unsafe_read() {
        let allocation = Allocation::with_size_align(32, 16).expect("allocation");
        for i in 0..allocation.len() {
            unsafe { allocation.as_ptr().add(i).write((i % 251) as u8) };
        }

        let bytes = unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
        assert_eq!(bytes[0], 0);
        assert_eq!(bytes[31], 31);
    }

    #[test]
    fn zero_sized_allocation_does_not_call_allocator() {
        for align in [1usize, 2, 4, 8, 16, 64, 4096] {
            let layout = Layout::from_size_align(0, align).expect("layout");
            let allocation = Allocation::new(layout).expect("allocation");
            assert!(allocation.is_empty());
            assert_eq!(allocation.layout(), layout);
            assert_eq!(allocation.as_ptr() as usize % align, 0);
        }
    }

    #[test]
    fn allocation_is_move_only_and_drops_once() {
        let allocation = Allocation::with_size_align(64, 16).expect("allocation");
        let ptr = allocation.as_ptr();
        unsafe { ptr.write(42) };

        let moved = allocation;
        assert_eq!(moved.as_ptr(), ptr);
        assert_eq!(unsafe { moved.as_ptr().read() }, 42);
    }

    #[test]
    fn allocation_has_expected_thread_traits() {
        fn assert_traits<T: Send + Sync>() {}
        assert_traits::<Allocation>();
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

    #[test]
    fn overflow_layout_reports_rust_error_without_allocating() {
        let err = Allocation::zeroed_with_size_align(usize::MAX, 2).expect_err("invalid layout");
        match err {
            TryAllocError::Layout(err) => {
                assert_eq!(err.size(), usize::MAX);
                assert_eq!(err.align(), 2);
            }
            TryAllocError::Alloc(_) => panic!("expected layout error"),
        }
    }

    #[test]
    fn error_types_are_copy_clone_and_format_without_owned_strings() {
        fn assert_traits<T: Copy + Clone + core::fmt::Debug + Eq>() {}
        assert_traits::<AllocError>();
        assert_traits::<LayoutError>();
        assert_traits::<TryAllocError>();

        let layout = Layout::from_size_align(64, 16).expect("layout");
        let alloc = AllocError { layout };
        let layout_err = LayoutError { size: 1, align: 3 };
        let try_alloc = TryAllocError::Alloc(alloc);

        let mut buf = FixedBuf::new();
        write!(&mut buf, "{alloc}; {layout_err}; {try_alloc}").expect("format");
        assert!(buf.as_str().contains("mimalloc allocation failed"));
        assert!(buf.as_str().contains("invalid allocation layout"));
    }

    struct FixedBuf {
        bytes: [u8; 256],
        len: usize,
    }

    impl FixedBuf {
        fn new() -> Self {
            Self {
                bytes: [0; 256],
                len: 0,
            }
        }

        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.bytes[..self.len]).expect("utf8")
        }
    }

    impl Write for FixedBuf {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let next = self.len.checked_add(s.len()).ok_or(fmt::Error)?;
            if next > self.bytes.len() {
                return Err(fmt::Error);
            }
            self.bytes[self.len..next].copy_from_slice(s.as_bytes());
            self.len = next;
            Ok(())
        }
    }
}
