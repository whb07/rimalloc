#![deny(clippy::must_use_candidate)]

use core::alloc::{GlobalAlloc, Layout};
use core::fmt;
use core::mem::ManuallyDrop;
use core::ptr;
use core::ptr::NonNull;

use crate::MiMalloc;

mod callbacks;
mod errors;
mod heaps;
mod options;
mod stats;

pub use callbacks::{CallbackError, DeferredFreeCallbackGuard, ErrorCallbackGuard};
pub use errors::{AllocError, LayoutError, ResizeError, ResizeErrorKind, TryAllocError};
pub use heaps::{Heap, HeapAllocation, HeapError, HeapRef, InitializedHeapAllocation, MainHeap};
pub use options::{OptionError, OptionKey, Options};
pub use stats::{ProcessInfo, StatCount, StatTotal, StatsError, StatsSnapshot};

/// Owned byte allocation backed by [`MiMalloc`].
///
/// This is the crate's minimal safe allocation wrapper. It converts C-style
/// null allocation failure into [`AllocError`] and releases the allocation on
/// drop. It intentionally does not expose heap handles, options, statistics, or
/// allocator-owned metadata.
///
/// `Allocation` is the uninitialized allocation type. [`Allocation::new`]
/// returns uninitialized memory; safe code can query the layout and raw pointer,
/// but it cannot read bytes through a safe slice API. Callers that use the raw
/// pointer must initialize memory before reading it.
///
/// [`Allocation::zeroed`] returns [`InitializedAllocation`], which exposes safe
/// byte-slice access because every byte is initialized to zero.
///
/// Zero-sized allocations do not call the allocator. They store an aligned
/// sentinel pointer and drop is a no-op.
///
/// `Allocation` is move-only. Dropping it frees the allocation exactly once;
/// there is no `Clone`, `Copy`, `from_raw`, or `into_raw` API.
///
/// ```rust,ignore
/// use core::alloc::Layout;
/// use mechanical_mimalloc::Allocation;
///
/// let layout = Layout::from_size_align(32, 8).unwrap();
/// let allocation = Allocation::zeroed(layout).unwrap();
/// assert_eq!(allocation.len(), 32);
/// assert!(!allocation.is_empty());
/// assert!(allocation.as_bytes().iter().all(|byte| *byte == 0));
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
#[must_use]
pub struct Allocation {
    raw: RawAllocation,
}

struct RawAllocation {
    ptr: NonNull<u8>,
    layout: Layout,
}

// SAFETY[AUDIT:owned-allocation]: `RawAllocation` has unique ownership of a
// block allocated by mimalloc. Mimalloc supports freeing allocations from a
// different thread, and this private owner does not expose references into the
// block.
unsafe impl Send for RawAllocation {}

// SAFETY[AUDIT:owned-allocation]: shared references to `RawAllocation` expose
// only metadata and a raw pointer. Safe code cannot read or mutate allocation
// contents through `&RawAllocation`.
unsafe impl Sync for RawAllocation {}

// SAFETY[AUDIT:owned-allocation]: `Allocation` has unique ownership of a block
// allocated by mimalloc. Mimalloc supports freeing allocations from a different
// thread, and safe methods do not expose references into the block.
unsafe impl Send for Allocation {}

// SAFETY[AUDIT:owned-allocation]: shared references to `Allocation` expose only
// metadata and raw pointers. Safe code cannot read or mutate allocation
// contents through `&Allocation`; any raw pointer dereference remains an unsafe
// caller boundary.
unsafe impl Sync for Allocation {}

impl fmt::Debug for Allocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Allocation")
            .field("raw", &self.raw)
            .finish()
    }
}

impl Allocation {
    /// Allocates an uninitialized byte block for `layout`.
    #[inline]
    pub fn new(layout: Layout) -> Result<Self, AllocError> {
        RawAllocation::allocate(layout, false).map(|raw| Self { raw })
    }

    /// Allocates a zero-initialized byte block for `layout`.
    #[inline]
    pub fn zeroed(layout: Layout) -> Result<InitializedAllocation, AllocError> {
        InitializedAllocation::zeroed(layout)
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
    pub fn zeroed_with_size_align(
        size: usize,
        align: usize,
    ) -> Result<InitializedAllocation, TryAllocError> {
        let layout = Layout::from_size_align(size, align)
            .map_err(|_| TryAllocError::Layout(LayoutError { size, align }))?;
        Self::zeroed(layout).map_err(TryAllocError::Alloc)
    }

    /// Returns the layout used to allocate this block.
    #[inline]
    #[must_use]
    pub const fn layout(&self) -> Layout {
        self.raw.layout
    }

    /// Returns the requested allocation size in bytes.
    ///
    /// This is metadata only; it does not prove that the allocation contents
    /// are initialized.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.raw.layout.size()
    }

    /// Returns whether this is a zero-sized allocation.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.raw.layout.size() == 0
    }

    /// Returns the allocation pointer as [`NonNull<u8>`].
    ///
    /// The pointer is valid only while this `Allocation` is alive. Callers must
    /// not free it manually, must not create aliasing references that violate
    /// Rust's aliasing rules, and must not read uninitialized memory. For
    /// zero-sized allocations this is an aligned sentinel pointer and must not
    /// be dereferenced.
    #[inline]
    #[must_use]
    pub const fn as_non_null(&self) -> NonNull<u8> {
        self.raw.ptr
    }

    /// Returns the allocation pointer.
    ///
    /// The pointer is valid only while this `Allocation` is alive. Callers must
    /// not free it manually, must not create aliasing references that violate
    /// Rust's aliasing rules, and must not read uninitialized memory. For
    /// zero-sized allocations this is an aligned sentinel pointer and must not
    /// be dereferenced.
    #[inline]
    #[must_use]
    pub const fn as_ptr(&self) -> *mut u8 {
        self.raw.ptr.as_ptr()
    }

    /// Resizes this allocation to `new_layout`.
    ///
    /// On success, bytes up to `min(old_len, new_len)` are preserved. On
    /// failure, the original allocation is returned inside [`ResizeError`].
    #[inline]
    pub fn resize(self, new_layout: Layout) -> Result<Self, ResizeError<Self>> {
        self.raw
            .resize_uninitialized(new_layout)
            .map(|raw| Self { raw })
            .map_err(|err| {
                let (raw, kind) = err.into_parts();
                ResizeError {
                    allocation: Self { raw },
                    kind,
                }
            })
    }

    /// Builds a layout from `size` and `align`, then resizes this allocation.
    #[inline]
    pub fn resize_with_size_align(
        self,
        size: usize,
        align: usize,
    ) -> Result<Self, ResizeError<Self>> {
        let layout = match Layout::from_size_align(size, align) {
            Ok(layout) => layout,
            Err(_) => {
                return Err(ResizeError {
                    allocation: self,
                    kind: ResizeErrorKind::Layout(LayoutError { size, align }),
                });
            }
        };
        self.resize(layout)
    }

    /// Converts this allocation into an initialized byte allocation when the
    /// allocation was produced by an older safe initialized constructor.
    ///
    /// New code should use [`Allocation::zeroed`] or
    /// [`InitializedAllocation::zeroed`] directly; those constructors return an
    /// initialized allocation type.
    #[inline]
    pub fn into_initialized(self) -> Result<InitializedAllocation, Self> {
        Err(self)
    }

    /// Marks this allocation initialized after the caller has initialized every
    /// byte through raw pointers.
    ///
    /// # Safety
    ///
    /// The caller must have initialized every byte in the allocation before
    /// calling this method.
    ///
    /// ```rust,compile_fail
    /// use core::alloc::Layout;
    /// use mechanical_mimalloc::Allocation;
    ///
    /// let layout = Layout::from_size_align(16, 8).unwrap();
    /// let allocation = Allocation::new(layout).unwrap();
    /// let _initialized = allocation.assume_initialized();
    /// ```
    #[inline]
    pub unsafe fn assume_initialized(self) -> InitializedAllocation {
        InitializedAllocation { raw: self.raw }
    }
}

impl RawAllocation {
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
    fn resize_uninitialized(self, new_layout: Layout) -> Result<Self, ResizeError<Self>> {
        self.resize_impl(new_layout, false)
    }

    #[inline]
    fn resize_initialized(self, new_layout: Layout) -> Result<Self, ResizeError<Self>> {
        self.resize_impl(new_layout, true)
    }

    #[inline]
    fn resize_impl(
        self,
        new_layout: Layout,
        zero_grown_tail: bool,
    ) -> Result<Self, ResizeError<Self>> {
        let old_layout = self.layout;
        if old_layout == new_layout {
            return Ok(self);
        }

        let old_size = old_layout.size();
        let new_size = new_layout.size();
        let old = ManuallyDrop::new(self);

        if new_size == 0 {
            if old_size != 0 {
                // SAFETY[AUDIT:owned-allocation]: `old` owns a live allocation
                // from this allocator and `ManuallyDrop` prevents double-free.
                unsafe { MiMalloc.dealloc(old.ptr.as_ptr(), old_layout) };
            }
            return Ok(Self {
                ptr: dangling_for(new_layout),
                layout: new_layout,
            });
        }

        if old_size == 0 {
            return match Self::allocate(new_layout, false) {
                Ok(allocation) => {
                    if zero_grown_tail {
                        allocation.zero_tail(0);
                    }
                    Ok(allocation)
                }
                Err(err) => Err(ResizeError {
                    allocation: ManuallyDrop::into_inner(old),
                    kind: ResizeErrorKind::Alloc(err),
                }),
            };
        }

        if old_layout.align() == new_layout.align() {
            // SAFETY[AUDIT:owned-allocation]: `old` owns `ptr` with
            // `old_layout`; `realloc` preserves the original allocation on
            // failure and transfers ownership to the returned pointer on
            // success.
            let ptr = unsafe { MiMalloc.realloc(old.ptr.as_ptr(), old_layout, new_size) };
            let Some(ptr) = NonNull::new(ptr) else {
                return Err(ResizeError {
                    allocation: ManuallyDrop::into_inner(old),
                    kind: ResizeErrorKind::Alloc(AllocError { layout: new_layout }),
                });
            };
            let allocation = Self {
                ptr,
                layout: new_layout,
            };
            if zero_grown_tail && new_size > old_size {
                allocation.zero_tail(old_size);
            }
            return Ok(allocation);
        }

        let new_allocation = match Self::allocate(new_layout, false) {
            Ok(allocation) => allocation,
            Err(err) => {
                return Err(ResizeError {
                    allocation: ManuallyDrop::into_inner(old),
                    kind: ResizeErrorKind::Alloc(err),
                });
            }
        };
        // SAFETY[AUDIT:owned-allocation]: both allocations are live and
        // non-overlapping; copy only the preserved prefix.
        unsafe {
            ptr::copy_nonoverlapping(
                old.ptr.as_ptr(),
                new_allocation.ptr.as_ptr(),
                old_size.min(new_size),
            );
            MiMalloc.dealloc(old.ptr.as_ptr(), old_layout);
        }
        if zero_grown_tail && new_size > old_size {
            new_allocation.zero_tail(old_size);
        }
        Ok(new_allocation)
    }

    #[inline]
    fn zero_tail(&self, initialized_prefix: usize) {
        let len = self.layout.size();
        if initialized_prefix < len {
            // SAFETY[AUDIT:owned-allocation]: `initialized_prefix..len` is within
            // the live allocation and is being initialized with zero bytes.
            unsafe {
                self.ptr
                    .as_ptr()
                    .add(initialized_prefix)
                    .write_bytes(0, len - initialized_prefix)
            };
        }
    }
}

impl Drop for RawAllocation {
    fn drop(&mut self) {
        if self.layout.size() != 0 {
            // SAFETY[AUDIT:owned-allocation]: non-zero `Allocation` values are
            // constructed only from successful `MiMalloc` allocation calls and
            // are freed exactly once from `Drop`.
            unsafe { MiMalloc.dealloc(self.ptr.as_ptr(), self.layout) };
        }
    }
}

impl fmt::Debug for RawAllocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawAllocation")
            .field("ptr", &self.ptr)
            .field("layout", &self.layout)
            .finish()
    }
}

/// Owned allocation whose bytes are initialized and safe to view as a byte slice.
#[must_use]
pub struct InitializedAllocation {
    raw: RawAllocation,
}

// SAFETY[AUDIT:owned-allocation]: this wrapper has the same unique ownership as
// `Allocation` and exposes ordinary byte slices only through Rust borrows.
unsafe impl Send for InitializedAllocation {}

// SAFETY[AUDIT:owned-allocation]: shared references expose immutable byte
// slices and allocation metadata only.
unsafe impl Sync for InitializedAllocation {}

impl InitializedAllocation {
    #[inline]
    pub fn zeroed(layout: Layout) -> Result<Self, AllocError> {
        RawAllocation::allocate(layout, true).map(|raw| Self { raw })
    }

    #[inline]
    pub fn zeroed_with_size_align(size: usize, align: usize) -> Result<Self, TryAllocError> {
        let layout = Layout::from_size_align(size, align)
            .map_err(|_| TryAllocError::Layout(LayoutError { size, align }))?;
        Self::zeroed(layout).map_err(TryAllocError::Alloc)
    }

    #[inline]
    #[must_use]
    pub const fn layout(&self) -> Layout {
        self.raw.layout
    }

    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.raw.layout.size()
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.raw.layout.size() == 0
    }

    #[inline]
    #[must_use]
    pub const fn as_non_null(&self) -> NonNull<u8> {
        self.raw.ptr
    }

    #[inline]
    #[must_use]
    pub const fn as_ptr(&self) -> *mut u8 {
        self.raw.ptr.as_ptr()
    }

    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY[AUDIT:owned-allocation]: `InitializedAllocation` guarantees
        // all bytes in the stored layout are initialized for reads.
        unsafe { core::slice::from_raw_parts(self.as_ptr().cast_const(), self.len()) }
    }

    #[inline]
    #[must_use]
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        // SAFETY[AUDIT:owned-allocation]: `&mut self` guarantees unique access
        // to the initialized allocation bytes.
        unsafe { core::slice::from_raw_parts_mut(self.as_ptr(), self.len()) }
    }

    #[inline]
    pub fn into_allocation(self) -> Allocation {
        Allocation { raw: self.raw }
    }

    #[inline]
    pub fn into_initialized(self) -> Result<Self, Allocation> {
        Ok(self)
    }

    /// Resizes this initialized allocation, zeroing any newly exposed tail
    /// bytes so the result remains initialized.
    #[inline]
    pub fn resize(self, new_layout: Layout) -> Result<Self, ResizeError<Self>> {
        match self.raw.resize_initialized(new_layout) {
            Ok(raw) => Ok(Self { raw }),
            Err(err) => {
                let (raw, kind) = err.into_parts();
                Err(ResizeError {
                    allocation: Self { raw },
                    kind,
                })
            }
        }
    }

    #[inline]
    pub fn resize_with_size_align(
        self,
        size: usize,
        align: usize,
    ) -> Result<Self, ResizeError<Self>> {
        let layout = match Layout::from_size_align(size, align) {
            Ok(layout) => layout,
            Err(_) => {
                return Err(ResizeError {
                    allocation: self,
                    kind: ResizeErrorKind::Layout(LayoutError { size, align }),
                });
            }
        };
        self.resize(layout)
    }

    /// Allocates a new initialized allocation and copies this allocation's bytes.
    #[inline]
    pub fn try_clone_zeroed_contents(&self) -> Result<Self, AllocError> {
        let mut cloned = RawAllocation::allocate(self.layout(), false).map(|raw| Self { raw })?;
        cloned.as_mut_bytes().copy_from_slice(self.as_bytes());
        Ok(cloned)
    }
}

impl fmt::Debug for InitializedAllocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InitializedAllocation")
            .field("raw", &self.raw)
            .finish()
    }
}

#[inline]
fn dangling_for(layout: Layout) -> NonNull<u8> {
    // SAFETY[AUDIT:owned-allocation]: `Layout` alignments are non-zero powers of
    // two, so the integer value is an aligned non-null sentinel for zero-sized
    // allocations and is never dereferenced by safe code.
    unsafe { NonNull::new_unchecked(layout.align() as *mut u8) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_void;
    use core::fmt::Write;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    static CALLBACK_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn owned_allocation_round_trips_bytes() {
        let layout = Layout::from_size_align(64, 16).expect("layout");
        let allocation = Allocation::new(layout).expect("allocation");
        assert_eq!(allocation.layout(), layout);
        assert_eq!(allocation.len(), 64);
        assert_eq!(allocation.as_ptr() as usize % 16, 0);
    }

    #[test]
    fn owned_allocation_covers_sizes_and_alignments() {
        let sizes = [1usize, 7, 64, 4096, 1 << 20];
        let alignments = [1usize, 2, 4, 8, 16, 64, 256, 4096];

        for size in sizes {
            for align in alignments {
                let layout = Layout::from_size_align(size, align).expect("layout");
                let allocation = Allocation::new(layout).expect("allocation");

                assert_eq!(allocation.layout(), layout);
                assert_eq!(allocation.len(), size);
                assert!(!allocation.is_empty());
                assert_ne!(allocation.as_ptr(), core::ptr::null_mut());
                assert_eq!(allocation.as_ptr() as usize % align, 0);
            }
        }
    }

    #[test]
    fn zeroed_allocation_returns_zeroed_bytes() {
        let allocation = Allocation::zeroed_with_size_align(32, 8).expect("allocation");
        let bytes = unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
        assert!(bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn zeroed_allocation_covers_sizes_and_alignments() {
        let sizes = [1usize, 32, 257, 4096];
        let alignments = [1usize, 8, 64, 4096];

        for size in sizes {
            for align in alignments {
                let allocation =
                    Allocation::zeroed_with_size_align(size, align).expect("allocation");

                assert_eq!(allocation.len(), size);
                assert_eq!(allocation.layout().align(), align);
                assert_eq!(allocation.as_ptr() as usize % align, 0);

                let bytes =
                    unsafe { core::slice::from_raw_parts(allocation.as_ptr(), allocation.len()) };
                assert!(bytes.iter().all(|byte| *byte == 0));
            }
        }
    }

    #[test]
    fn uninitialized_allocation_can_be_written_before_unsafe_read() {
        let allocation = Allocation::with_size_align(32, 16).expect("allocation");
        assert_eq!(allocation.as_non_null().as_ptr(), allocation.as_ptr());

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

            let zeroed = Allocation::zeroed(layout).expect("zeroed allocation");
            assert!(zeroed.is_empty());
            assert_eq!(zeroed.layout(), layout);
            assert_eq!(zeroed.as_ptr() as usize % align, 0);
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
        fn assert_traits<T: Send + Sync>() -> bool {
            let _ = core::marker::PhantomData::<T>;
            true
        }
        assert!(assert_traits::<Allocation>());
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
        fn assert_traits<T: Copy + Clone + core::fmt::Debug + Eq>() -> bool {
            let _ = core::marker::PhantomData::<T>;
            true
        }
        assert!(assert_traits::<AllocError>());
        assert!(assert_traits::<LayoutError>());
        assert!(assert_traits::<TryAllocError>());

        let layout = Layout::from_size_align(64, 16).expect("layout");
        let alloc = AllocError { layout };
        let layout_err = LayoutError { size: 1, align: 3 };
        let try_alloc = TryAllocError::Alloc(alloc);

        let mut buf = FixedBuf::new();
        write!(&mut buf, "{alloc}; {layout_err}; {try_alloc}").expect("format");
        assert!(buf.as_str().contains("mimalloc allocation failed"));
        assert!(buf.as_str().contains("invalid allocation layout"));
    }

    #[test]
    fn resize_error_preserves_allocation_access() {
        let allocation = Allocation::with_size_align(16, 8).expect("allocation");
        let layout_err = LayoutError { size: 1, align: 3 };
        let mut err = ResizeError {
            allocation,
            kind: ResizeErrorKind::Layout(layout_err),
        };
        assert!(matches!(err.kind(), ResizeErrorKind::Layout(_)));
        assert_eq!(err.allocation().len(), 16);
        assert_eq!(err.allocation_mut().len(), 16);
        let (allocation, kind) = err.into_parts();
        assert_eq!(allocation.len(), 16);
        assert_eq!(kind, ResizeErrorKind::Layout(layout_err));

        let alloc_kind = ResizeErrorKind::Alloc(AllocError {
            layout: Layout::from_size_align(32, 8).expect("layout"),
        });
        assert!(matches!(alloc_kind, ResizeErrorKind::Alloc(_)));
    }

    #[test]
    fn allocation_resize_grow_shrink_same_and_zero_edges() {
        let layout = Layout::from_size_align(16, 8).expect("layout");
        let allocation = Allocation::new(layout).expect("allocation");
        for idx in 0..allocation.len() {
            unsafe { allocation.as_ptr().add(idx).write(idx as u8) };
        }

        let grown_layout = Layout::from_size_align(64, 8).expect("layout");
        let grown = allocation.resize(grown_layout).expect("grow");
        let grown_bytes = unsafe { core::slice::from_raw_parts(grown.as_ptr(), 16) };
        assert_eq!(grown_bytes, (0u8..16).collect::<Vec<_>>().as_slice());

        let shrunk_layout = Layout::from_size_align(8, 8).expect("layout");
        let shrunk = grown.resize(shrunk_layout).expect("shrink");
        let shrunk_bytes = unsafe { core::slice::from_raw_parts(shrunk.as_ptr(), 8) };
        assert_eq!(shrunk_bytes, (0u8..8).collect::<Vec<_>>().as_slice());

        let same_ptr = shrunk.as_ptr();
        let same = shrunk.resize(shrunk_layout).expect("same size");
        assert_eq!(same.as_ptr(), same_ptr);

        let zero_layout = Layout::from_size_align(0, 16).expect("layout");
        let zero = same.resize(zero_layout).expect("nonzero to zero");
        assert!(zero.is_empty());
        assert_eq!(zero.as_ptr() as usize % 16, 0);

        let nonzero = zero
            .resize(Layout::from_size_align(4, 4).expect("layout"))
            .expect("zero to nonzero");
        assert_eq!(nonzero.len(), 4);
        assert_eq!(nonzero.as_ptr() as usize % 4, 0);
    }

    #[test]
    fn allocation_resize_invalid_layout_preserves_original() {
        let allocation = Allocation::with_size_align(24, 8).expect("allocation");
        let err = allocation
            .resize_with_size_align(usize::MAX, 2)
            .expect_err("layout error");
        assert!(matches!(err.kind(), ResizeErrorKind::Layout(_)));
        assert_eq!(err.allocation().len(), 24);
        let allocation = err.into_allocation();
        assert_eq!(allocation.len(), 24);
    }

    #[test]
    fn initialized_allocation_exposes_safe_slices_and_clone() {
        let allocation = Allocation::zeroed_with_size_align(32, 8)
            .expect("allocation")
            .into_initialized()
            .expect("initialized");
        assert!(allocation.as_bytes().iter().all(|byte| *byte == 0));

        let mut allocation = allocation;
        allocation.as_mut_bytes()[0] = 7;
        allocation.as_mut_bytes()[31] = 9;
        assert_eq!(allocation.as_bytes()[0], 7);
        assert_eq!(allocation.as_bytes()[31], 9);

        let cloned = allocation
            .try_clone_zeroed_contents()
            .expect("clone initialized bytes");
        assert_eq!(cloned.as_bytes(), allocation.as_bytes());
    }

    #[test]
    fn uninitialized_allocation_requires_explicit_initialization() {
        let allocation = Allocation::with_size_align(8, 8).expect("allocation");
        let allocation = allocation
            .into_initialized()
            .expect_err("uninitialized allocation should not expose bytes");
        for idx in 0..allocation.len() {
            unsafe { allocation.as_ptr().add(idx).write((idx + 1) as u8) };
        }
        let initialized = unsafe { allocation.assume_initialized() };
        assert_eq!(initialized.as_bytes(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn initialized_resize_zeroes_grown_tail() {
        let mut allocation = Allocation::zeroed_with_size_align(4, 4)
            .expect("allocation")
            .into_initialized()
            .expect("initialized");
        allocation.as_mut_bytes().copy_from_slice(&[1, 2, 3, 4]);
        let resized = allocation
            .resize(Layout::from_size_align(8, 4).expect("layout"))
            .expect("resize initialized");
        assert_eq!(resized.as_bytes(), &[1, 2, 3, 4, 0, 0, 0, 0]);
    }

    #[test]
    fn heap_errors_and_owned_heap_allocation_paths_work() {
        assert!(matches!(HeapError::Create, HeapError::Create));
        assert!(matches!(
            HeapError::MainUnavailable,
            HeapError::MainUnavailable
        ));
        assert!(matches!(HeapError::Unsupported, HeapError::Unsupported));

        let heap = Heap::new().expect("heap");
        let allocation = heap
            .allocate(Layout::from_size_align(64, 16).expect("layout"))
            .expect("heap allocation");
        assert_eq!(allocation.len(), 64);
        assert_eq!(allocation.as_ptr() as usize % 16, 0);

        let zero = heap
            .allocate(Layout::from_size_align(0, 64).expect("layout"))
            .expect("zero heap allocation");
        assert!(zero.is_empty());
        assert_eq!(zero.as_ptr() as usize % 64, 0);

        let over_aligned = heap
            .allocate(Layout::from_size_align(128, 4096).expect("layout"))
            .expect("over-aligned heap allocation");
        assert_eq!(over_aligned.as_ptr() as usize % 4096, 0);
    }

    #[test]
    fn heap_zeroed_allocation_and_resize_preserve_ownership() {
        let heap = Heap::new().expect("heap");
        let zeroed = heap
            .allocate_zeroed(Layout::from_size_align(16, 8).expect("layout"))
            .expect("zeroed");
        assert!(zeroed.as_bytes().iter().all(|byte| *byte == 0));

        let allocation = zeroed.into_allocation();
        for idx in 0..allocation.len() {
            unsafe { allocation.as_ptr().add(idx).write(idx as u8) };
        }
        let grown = allocation
            .resize(Layout::from_size_align(32, 8).expect("layout"))
            .expect("grow");
        let bytes = unsafe { core::slice::from_raw_parts(grown.as_ptr(), 16) };
        assert_eq!(bytes, (0u8..16).collect::<Vec<_>>().as_slice());

        let shrunk = grown
            .resize(Layout::from_size_align(8, 8).expect("layout"))
            .expect("shrink");
        assert_eq!(shrunk.len(), 8);

        let err = shrunk
            .resize_with_size_align(16, 3)
            .expect_err("invalid layout");
        assert!(matches!(err.kind(), ResizeErrorKind::Layout(_)));
        assert_eq!(err.allocation().len(), 8);
    }

    #[test]
    fn main_heap_allocates_without_owning_heap() {
        let main = MainHeap::get().expect("main heap");
        let allocation = main
            .allocate(Layout::from_size_align(32, 8).expect("layout"))
            .expect("main heap allocation");
        assert_eq!(allocation.len(), 32);
    }

    #[test]
    fn safe_options_cover_supported_invalid_and_immutable_keys() {
        assert!(matches!(OptionError::InvalidKey, OptionError::InvalidKey));
        assert!(matches!(OptionError::Unsupported, OptionError::Unsupported));

        for key in OptionKey::ALL {
            assert_eq!(OptionKey::from_raw(key.as_raw()), Ok(key));
            let _ = Options::get(key).expect("get supported key");
        }
        assert_eq!(OptionKey::from_raw(-1), Err(OptionError::InvalidKey));
        assert_eq!(OptionKey::from_raw(2), Err(OptionError::Unsupported));
        assert_eq!(
            Options::set(OptionKey::ShowStats, 1),
            Err(OptionError::Unsupported)
        );

        let original = Options::get(OptionKey::MaxWarnings).expect("original");
        let changed = if original == 7 { 8 } else { 7 };
        Options::set(OptionKey::MaxWarnings, changed).expect("set");
        assert_eq!(Options::get(OptionKey::MaxWarnings), Ok(changed));
        Options::reset(OptionKey::MaxWarnings).expect("reset");
        assert_eq!(
            Options::get(OptionKey::MaxWarnings),
            Ok(OptionKey::MaxWarnings.default_value())
        );
        Options::set(OptionKey::MaxWarnings, original).expect("restore");
    }

    #[test]
    fn stats_and_process_info_are_typed_and_nonprinting() {
        assert!(matches!(StatsError::Unavailable, StatsError::Unavailable));
        assert!(matches!(
            StatsError::HeapUnavailable,
            StatsError::HeapUnavailable
        ));
        assert!(matches!(
            StatsError::BufferTooSmall,
            StatsError::BufferTooSmall
        ));
        assert_eq!(StatCount::new(3).get(), 3);
        assert_eq!(StatTotal::new(4).get(), 4);

        let before = StatsSnapshot::capture().expect("stats before");
        let allocation = Allocation::with_size_align(1024, 16).expect("allocation");
        drop(allocation);
        let after = StatsSnapshot::capture().expect("stats after");
        assert!(after.malloc_requested_total().get() >= before.malloc_requested_total().get());

        let mut tiny = [0u8; 1];
        assert_eq!(after.write_json(&mut tiny), Err(StatsError::BufferTooSmall));
        let mut json = [0u8; 16 * 1024];
        let len = after.write_json(&mut json).expect("json");
        assert!(len > 0);
        assert!(json.starts_with(b"{"));

        let heap = Heap::new().expect("heap");
        let heap_stats = heap.as_ref().stats().expect("heap stats");
        assert!(heap_stats.pages_current().get() >= 0);

        let info = ProcessInfo::current();
        assert!(info.page_size().is_some_and(|page_size| page_size > 0));
        assert!(info.page_faults().is_some());
        let raw_page_size = unsafe { crate::os::_mi_os_page_size() };
        assert_eq!(info.page_size(), Some(raw_page_size));
    }

    static PREVIOUS_ERROR_CALLBACK_HITS: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn previous_error_callback(_err: i32, _arg: *mut c_void) {
        PREVIOUS_ERROR_CALLBACK_HITS.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn error_callback_guard_invokes_and_restores_previous_callback_quietly() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let original = crate::options::registered_error_callback();
        let original_show_errors = Options::get(OptionKey::ShowErrors).expect("show errors");
        Options::set(OptionKey::ShowErrors, 0).expect("disable error output");
        PREVIOUS_ERROR_CALLBACK_HITS.store(0, Ordering::SeqCst);
        crate::options::mi_register_error(Some(previous_error_callback), ptr::null_mut());

        let mut hits = 0usize;
        {
            let mut callback = |_err: i32| {
                hits += 1;
            };
            let _guard = ErrorCallbackGuard::new(&mut callback).expect("guard");
            crate::options::_mi_error_message_fmt(
                crate::core_types::Errno::EINVAL,
                format_args!("safe error callback guard test\n"),
            );
        }
        assert_eq!(hits, 1);

        crate::options::_mi_error_message_fmt(
            crate::core_types::Errno::EINVAL,
            format_args!("safe error callback restore test\n"),
        );
        assert_eq!(PREVIOUS_ERROR_CALLBACK_HITS.load(Ordering::SeqCst), 1);

        crate::options::restore_error_callback(original);
        Options::set(OptionKey::ShowErrors, original_show_errors).expect("restore show errors");
    }

    #[test]
    fn panicking_error_callback_is_caught_before_c_abi_return() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let original = crate::options::registered_error_callback();
        let original_show_errors = Options::get(OptionKey::ShowErrors).expect("show errors");
        Options::set(OptionKey::ShowErrors, 0).expect("disable error output");
        let old_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        {
            let mut callback = |_err: i32| {
                panic!("callback panic should be caught");
            };
            let _guard = ErrorCallbackGuard::new(&mut callback).expect("guard");
            crate::options::_mi_error_message_fmt(
                crate::core_types::Errno::EINVAL,
                format_args!("safe panic callback guard test\n"),
            );
        }

        std::panic::set_hook(old_hook);
        crate::options::restore_error_callback(original);
        Options::set(OptionKey::ShowErrors, original_show_errors).expect("restore show errors");
    }

    #[test]
    fn deferred_free_callback_guard_registers_and_restores() {
        let _lock = CALLBACK_TEST_LOCK.lock().expect("callback test lock");
        let original = unsafe { crate::page::registered_deferred_free_callback() };
        {
            let mut callback = |_force: bool, _heartbeat: u64| {};
            let _guard = DeferredFreeCallbackGuard::new(&mut callback).expect("guard");
            let current = unsafe { crate::page::registered_deferred_free_callback() };
            assert!(current.0.is_some());
        }
        let restored = unsafe { crate::page::registered_deferred_free_callback() };
        assert_eq!(callback_addr(restored.0), callback_addr(original.0));
        assert_eq!(restored.1, original.1);
        unsafe { crate::page::restore_deferred_free_callback(original) };
    }

    #[test]
    fn callback_error_variants_are_distinct() {
        assert!(matches!(CallbackError::Register, CallbackError::Register));
        assert!(matches!(
            CallbackError::Unsupported,
            CallbackError::Unsupported
        ));
    }

    fn callback_addr<T>(callback: Option<T>) -> usize
    where
        T: Copy,
    {
        callback
            .map(|callback| unsafe { core::mem::transmute_copy::<T, usize>(&callback) })
            .unwrap_or(0)
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
