use core::alloc::{GlobalAlloc, Layout};
use core::fmt;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ptr;
use core::ptr::NonNull;

use crate::MiMalloc;

use super::{AllocError, LayoutError, ResizeError, ResizeErrorKind, TryAllocError};

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
/// ```rust,no_run
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

    /// Allocates an uninitialized byte block with alignment 1.
    #[inline]
    pub fn new_bytes(len: usize) -> Result<Self, TryAllocError> {
        Self::with_size_align(len, 1)
    }

    /// Allocates a zero-initialized byte block with alignment 1.
    #[inline]
    pub fn zeroed_bytes(len: usize) -> Result<InitializedAllocation, TryAllocError> {
        Self::zeroed_with_size_align(len, 1)
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
    pub const fn as_ptr(&self) -> *const u8 {
        self.raw.ptr.as_ptr().cast_const()
    }

    /// Returns the allocation pointer for mutation.
    ///
    /// The pointer is valid only while this `Allocation` is alive. Callers must
    /// initialize bytes before reading them, and zero-sized allocation pointers
    /// remain aligned sentinels that must not be dereferenced.
    #[inline]
    #[must_use]
    pub const fn as_mut_ptr(&mut self) -> *mut u8 {
        self.raw.ptr.as_ptr()
    }

    /// Returns this uninitialized allocation as a `MaybeUninit<u8>` slice.
    ///
    /// The slice length is the requested allocation size. Zero-sized
    /// allocations return an empty slice and do not dereference the sentinel
    /// pointer.
    #[inline]
    #[must_use]
    pub fn as_uninit_slice(&self) -> &[MaybeUninit<u8>] {
        // SAFETY[AUDIT:owned-allocation]: `self.raw.ptr` is valid for
        // `self.len()` bytes when non-zero. `MaybeUninit<u8>` permits
        // uninitialized contents, and zero-sized allocations produce an empty
        // slice over the aligned sentinel.
        unsafe { core::slice::from_raw_parts(self.as_ptr().cast(), self.len()) }
    }

    /// Returns this uninitialized allocation as a mutable `MaybeUninit<u8>`
    /// slice.
    ///
    /// Use this to initialize bytes without raw pointer loops, then call the
    /// explicit unsafe [`Allocation::assume_initialized`] transition after
    /// every byte has been initialized.
    #[inline]
    #[must_use]
    pub fn as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        // SAFETY[AUDIT:owned-allocation]: `&mut self` guarantees unique access
        // to the allocation bytes. `MaybeUninit<u8>` permits writing
        // uninitialized storage, and zero-sized allocations return an empty
        // slice over the aligned sentinel.
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr().cast(), self.len()) }
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
        let Ok(layout) = Layout::from_size_align(size, align) else {
            return Err(ResizeError {
                allocation: self,
                kind: ResizeErrorKind::Layout(LayoutError { size, align }),
            });
        };
        self.resize(layout)
    }

    /// Marks this allocation initialized after the caller has initialized every
    /// byte through raw pointers.
    ///
    /// # Safety
    ///
    /// The caller must guarantee every byte in the allocation has been
    /// initialized before calling this method.
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

        #[cfg(test)]
        if super::failure_injection::should_fail_allocation() {
            return Err(AllocError { layout });
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
            #[cfg(test)]
            if super::failure_injection::should_fail_allocation() {
                return Err(ResizeError {
                    allocation: ManuallyDrop::into_inner(old),
                    kind: ResizeErrorKind::Alloc(AllocError { layout: new_layout }),
                });
            }

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
///
/// ```rust
/// use mechanical_mimalloc::InitializedAllocation;
///
/// let mut allocation = InitializedAllocation::zeroed_bytes(4).unwrap();
/// assert_eq!(allocation.as_bytes(), &[0, 0, 0, 0]);
///
/// allocation.as_mut_bytes().copy_from_slice(&[1, 2, 3, 4]);
/// let cloned = allocation.try_clone_contents().unwrap();
/// assert_eq!(cloned.as_bytes(), &[1, 2, 3, 4]);
/// ```
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
    /// Allocates a zero-initialized byte block for `layout`.
    #[inline]
    pub fn zeroed(layout: Layout) -> Result<Self, AllocError> {
        RawAllocation::allocate(layout, true).map(|raw| Self { raw })
    }

    /// Allocates a zero-initialized byte block with alignment 1.
    #[inline]
    pub fn zeroed_bytes(len: usize) -> Result<Self, TryAllocError> {
        Self::zeroed_with_size_align(len, 1)
    }

    /// Builds a layout from `size` and `align`, then allocates a zeroed block.
    #[inline]
    pub fn zeroed_with_size_align(size: usize, align: usize) -> Result<Self, TryAllocError> {
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
    #[inline]
    #[must_use]
    pub const fn as_non_null(&self) -> NonNull<u8> {
        self.raw.ptr
    }

    /// Returns a shared raw pointer to the initialized bytes.
    #[inline]
    #[must_use]
    pub const fn as_ptr(&self) -> *const u8 {
        self.raw.ptr.as_ptr().cast_const()
    }

    /// Returns a mutable raw pointer to the initialized bytes.
    #[inline]
    #[must_use]
    pub const fn as_mut_ptr(&mut self) -> *mut u8 {
        self.raw.ptr.as_ptr()
    }

    /// Returns the initialized bytes as an immutable slice.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY[AUDIT:owned-allocation]: `InitializedAllocation` guarantees
        // all bytes in the stored layout are initialized for reads.
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
    }

    /// Returns the initialized bytes as a mutable slice.
    #[inline]
    #[must_use]
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        // SAFETY[AUDIT:owned-allocation]: `&mut self` guarantees unique access
        // to the allocation bytes.
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.len()) }
    }

    /// Converts this initialized wrapper back into an uninitialized allocation wrapper.
    #[inline]
    pub fn into_allocation(self) -> Allocation {
        Allocation { raw: self.raw }
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

    /// Builds a layout from `size` and `align`, then resizes this allocation.
    #[inline]
    pub fn resize_with_size_align(
        self,
        size: usize,
        align: usize,
    ) -> Result<Self, ResizeError<Self>> {
        let Ok(layout) = Layout::from_size_align(size, align) else {
            return Err(ResizeError {
                allocation: self,
                kind: ResizeErrorKind::Layout(LayoutError { size, align }),
            });
        };
        self.resize(layout)
    }

    /// Allocates a new initialized allocation and copies this allocation's bytes.
    #[inline]
    pub fn try_clone_contents(&self) -> Result<Self, AllocError> {
        let mut cloned = RawAllocation::allocate(self.layout(), false).map(|raw| Self { raw })?;
        cloned.as_mut_bytes().copy_from_slice(self.as_bytes());
        Ok(cloned)
    }
}

impl AsRef<[u8]> for InitializedAllocation {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsMut<[u8]> for InitializedAllocation {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_bytes()
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
pub(super) fn dangling_for(layout: Layout) -> NonNull<u8> {
    // SAFETY[AUDIT:owned-allocation]: `Layout` alignments are non-zero powers of
    // two, so the integer value is an aligned non-null sentinel for zero-sized
    // allocations and is never dereferenced by safe code.
    unsafe { NonNull::new_unchecked(layout.align() as *mut u8) }
}
