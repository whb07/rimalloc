use core::alloc::Layout;
use core::ffi::c_void;
use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ptr;
use core::ptr::NonNull;

use super::{
    AllocError, LayoutError, ResizeError, ResizeErrorKind, StatsError, StatsSnapshot,
    allocations::dangling_for,
};

/// Error returned by safe heap ownership APIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapError {
    Create,
    MainUnavailable,
    Unsupported,
}

impl fmt::Display for HeapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create => f.write_str("mimalloc heap creation failed"),
            Self::MainUnavailable => f.write_str("mimalloc main heap is unavailable"),
            Self::Unsupported => f.write_str("mimalloc heap operation is unsupported"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HeapError {}

/// Owned mimalloc heap.
///
/// Heap-tied allocations borrow the heap, so safe code cannot move them beyond
/// the heap lifetime:
///
/// ```rust,compile_fail
/// use core::alloc::Layout;
/// use mechanical_mimalloc::{Heap, HeapAllocation};
///
/// let layout = Layout::from_size_align(16, 8).unwrap();
/// let leaked: HeapAllocation<'static>;
/// {
///     let heap = Heap::new().unwrap();
///     leaked = heap.allocate(layout).unwrap();
/// }
/// drop(leaked);
/// ```
#[must_use]
pub struct Heap {
    ptr: NonNull<crate::types::mi_heap_t>,
}

impl Heap {
    #[inline]
    pub fn new() -> Result<Self, HeapError> {
        // SAFETY[AUDIT:safe-heap]: heap creation initializes allocator runtime
        // as needed and returns either null or an owned heap pointer.
        let ptr = unsafe { crate::heap::mi_heap_new() };
        let ptr = NonNull::new(ptr).ok_or(HeapError::Create)?;
        Ok(Self { ptr })
    }

    #[inline]
    pub fn as_ref(&self) -> HeapRef<'_> {
        HeapRef {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn allocate(&self, layout: Layout) -> Result<HeapAllocation<'_>, AllocError> {
        self.as_ref().allocate(layout)
    }

    #[inline]
    pub fn allocate_zeroed(
        &self,
        layout: Layout,
    ) -> Result<InitializedHeapAllocation<'_>, AllocError> {
        self.as_ref().allocate_zeroed(layout)
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        // SAFETY[AUDIT:safe-heap]: `Heap` owns this non-main heap and Rust
        // lifetimes prevent safe heap-tied allocations from outliving it.
        unsafe { crate::heap::mi_heap_delete(self.ptr.as_ptr()) };
    }
}

impl fmt::Debug for Heap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Heap").field("ptr", &self.ptr).finish()
    }
}

/// Borrowed heap handle used to tie heap allocations to a heap lifetime.
#[derive(Clone, Copy)]
#[must_use]
pub struct HeapRef<'heap> {
    pub(super) ptr: NonNull<crate::types::mi_heap_t>,
    _marker: PhantomData<&'heap Heap>,
}

impl<'heap> HeapRef<'heap> {
    #[inline]
    pub fn allocate(self, layout: Layout) -> Result<HeapAllocation<'heap>, AllocError> {
        HeapAllocation::allocate_in(self.ptr, layout, false)
    }

    #[inline]
    pub fn allocate_zeroed(
        self,
        layout: Layout,
    ) -> Result<InitializedHeapAllocation<'heap>, AllocError> {
        InitializedHeapAllocation::allocate_zeroed_in(self.ptr, layout)
    }

    #[inline]
    pub fn stats(self) -> Result<StatsSnapshot, StatsError> {
        StatsSnapshot::for_heap(self)
    }
}

impl fmt::Debug for HeapRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeapRef").field("ptr", &self.ptr).finish()
    }
}

/// Borrowed process main heap handle.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct MainHeap {
    ptr: NonNull<crate::types::mi_heap_t>,
}

impl MainHeap {
    #[inline]
    pub fn get() -> Result<Self, HeapError> {
        // SAFETY[AUDIT:safe-heap]: returns the process main heap or null if
        // runtime initialization is unavailable.
        let ptr = unsafe { crate::init::mi_heap_main() };
        let ptr = NonNull::new(ptr).ok_or(HeapError::MainUnavailable)?;
        Ok(Self { ptr })
    }

    #[inline]
    pub fn as_ref(&self) -> HeapRef<'_> {
        HeapRef {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn allocate(&self, layout: Layout) -> Result<HeapAllocation<'_>, AllocError> {
        self.as_ref().allocate(layout)
    }
}

/// Allocation tied to the lifetime of the heap borrow that created it.
///
/// ```rust,compile_fail
/// use core::alloc::Layout;
/// use mechanical_mimalloc::{Heap, HeapAllocation};
///
/// fn allocate_then_drop_heap() -> HeapAllocation<'static> {
///     let heap = Heap::new().unwrap();
///     heap.allocate(Layout::from_size_align(32, 8).unwrap()).unwrap()
/// }
/// ```
#[must_use]
pub struct HeapAllocation<'heap> {
    raw: RawHeapAllocation<'heap>,
}

struct RawHeapAllocation<'heap> {
    ptr: NonNull<u8>,
    layout: Layout,
    heap: NonNull<crate::types::mi_heap_t>,
    _marker: PhantomData<&'heap Heap>,
}

impl<'heap> HeapAllocation<'heap> {
    #[inline]
    fn allocate_in(
        heap: NonNull<crate::types::mi_heap_t>,
        layout: Layout,
        zeroed: bool,
    ) -> Result<Self, AllocError> {
        RawHeapAllocation::allocate_in(heap, layout, zeroed).map(|raw| Self { raw })
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
    pub const fn as_ptr(&self) -> *mut u8 {
        self.raw.ptr.as_ptr()
    }

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
}

impl<'heap> RawHeapAllocation<'heap> {
    #[inline]
    fn allocate_in(
        heap: NonNull<crate::types::mi_heap_t>,
        layout: Layout,
        zeroed: bool,
    ) -> Result<Self, AllocError> {
        if layout.size() == 0 {
            return Ok(Self {
                ptr: dangling_for(layout),
                layout,
                heap,
                _marker: PhantomData,
            });
        }

        // SAFETY[AUDIT:safe-heap]: `heap` is a live heap borrowed for
        // `'heap`; raw allocation returns null on failure.
        let ptr = unsafe {
            if layout.align() <= crate::types::MI_MAX_ALIGN_SIZE {
                if zeroed {
                    crate::alloc::mi_heap_zalloc(heap.as_ptr(), layout.size())
                } else {
                    crate::alloc::mi_heap_malloc(heap.as_ptr(), layout.size())
                }
            } else if zeroed {
                crate::alloc_aligned::mi_heap_zalloc_aligned(
                    heap.as_ptr(),
                    layout.size(),
                    layout.align(),
                )
            } else {
                crate::alloc_aligned::mi_heap_malloc_aligned(
                    heap.as_ptr(),
                    layout.size(),
                    layout.align(),
                )
            }
        }
        .cast::<u8>();
        let ptr = NonNull::new(ptr).ok_or(AllocError { layout })?;
        Ok(Self {
            ptr,
            layout,
            heap,
            _marker: PhantomData,
        })
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
                // SAFETY[AUDIT:safe-heap]: heap allocations are released
                // through mimalloc free and `ManuallyDrop` prevents double-free.
                unsafe { crate::mimalloc::mi_free(old.ptr.as_ptr().cast::<c_void>()) };
            }
            return Ok(Self {
                ptr: dangling_for(new_layout),
                layout: new_layout,
                heap: old.heap,
                _marker: PhantomData,
            });
        }

        if old_size != 0 && old_layout.align() == new_layout.align() {
            // SAFETY[AUDIT:safe-heap]: `old` owns this heap allocation and
            // mimalloc leaves it valid on realloc failure.
            let ptr = unsafe {
                if old_layout.align() <= crate::types::MI_MAX_ALIGN_SIZE {
                    crate::alloc::mi_heap_realloc(
                        old.heap.as_ptr(),
                        old.ptr.as_ptr().cast::<c_void>(),
                        new_size,
                    )
                } else {
                    crate::alloc_aligned::mi_heap_realloc_aligned(
                        old.heap.as_ptr(),
                        old.ptr.as_ptr().cast::<c_void>(),
                        new_size,
                        old_layout.align(),
                    )
                }
            }
            .cast::<u8>();
            let Some(ptr) = NonNull::new(ptr) else {
                return Err(ResizeError {
                    allocation: ManuallyDrop::into_inner(old),
                    kind: ResizeErrorKind::Alloc(AllocError { layout: new_layout }),
                });
            };
            let allocation = Self {
                ptr,
                layout: new_layout,
                heap: old.heap,
                _marker: PhantomData,
            };
            if zero_grown_tail && new_size > old_size {
                allocation.zero_tail(old_size);
            }
            return Ok(allocation);
        }

        let new_allocation = match Self::allocate_in(old.heap, new_layout, false) {
            Ok(allocation) => allocation,
            Err(err) => {
                return Err(ResizeError {
                    allocation: ManuallyDrop::into_inner(old),
                    kind: ResizeErrorKind::Alloc(err),
                });
            }
        };
        if old_size != 0 {
            // SAFETY[AUDIT:safe-heap]: both allocations are live and
            // non-overlapping; copy only preserved bytes and free the old block.
            unsafe {
                ptr::copy_nonoverlapping(
                    old.ptr.as_ptr(),
                    new_allocation.ptr.as_ptr(),
                    old_size.min(new_size),
                );
                crate::mimalloc::mi_free(old.ptr.as_ptr().cast::<c_void>());
            }
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
            // SAFETY[AUDIT:safe-heap]: `initialized_prefix..len` is inside the
            // live heap allocation and is being initialized with zero bytes.
            unsafe {
                self.ptr
                    .as_ptr()
                    .add(initialized_prefix)
                    .write_bytes(0, len - initialized_prefix)
            };
        }
    }
}

impl Drop for RawHeapAllocation<'_> {
    fn drop(&mut self) {
        if self.layout.size() != 0 {
            // SAFETY[AUDIT:safe-heap]: this value owns a live mimalloc block.
            unsafe { crate::mimalloc::mi_free(self.ptr.as_ptr().cast::<c_void>()) };
        }
    }
}

impl fmt::Debug for RawHeapAllocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawHeapAllocation")
            .field("ptr", &self.ptr)
            .field("layout", &self.layout)
            .field("heap", &self.heap)
            .finish()
    }
}

impl fmt::Debug for HeapAllocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeapAllocation")
            .field("raw", &self.raw)
            .finish()
    }
}

/// Heap allocation whose bytes are initialized.
#[must_use]
pub struct InitializedHeapAllocation<'heap> {
    raw: RawHeapAllocation<'heap>,
}

impl<'heap> InitializedHeapAllocation<'heap> {
    #[inline]
    fn allocate_zeroed_in(
        heap: NonNull<crate::types::mi_heap_t>,
        layout: Layout,
    ) -> Result<Self, AllocError> {
        RawHeapAllocation::allocate_in(heap, layout, true).map(|raw| Self { raw })
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
    pub const fn as_ptr(&self) -> *mut u8 {
        self.raw.ptr.as_ptr()
    }

    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY[AUDIT:safe-heap]: initialized heap allocation guarantees
        // readable initialized bytes.
        unsafe { core::slice::from_raw_parts(self.as_ptr().cast_const(), self.len()) }
    }

    #[inline]
    #[must_use]
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        // SAFETY[AUDIT:safe-heap]: `&mut self` guarantees unique access.
        unsafe { core::slice::from_raw_parts_mut(self.as_ptr(), self.len()) }
    }

    #[inline]
    pub fn into_allocation(self) -> HeapAllocation<'heap> {
        HeapAllocation { raw: self.raw }
    }

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
}

impl fmt::Debug for InitializedHeapAllocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InitializedHeapAllocation")
            .field("raw", &self.raw)
            .finish()
    }
}
