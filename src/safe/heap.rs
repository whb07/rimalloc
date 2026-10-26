use core::alloc::Layout;
use core::ffi::c_void;
use core::fmt;
use core::marker::PhantomData;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ptr;
use core::ptr::NonNull;

use super::{
    AllocError, LayoutError, ResizeError, ResizeErrorKind, StatsError, StatsSnapshot,
    TryAllocError, allocation::dangling_for, error::try_layout,
};

type NotSendSync = PhantomData<*mut ()>;

/// Error returned by safe heap ownership APIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HeapError {
    /// Mimalloc returned null while creating a heap.
    Create,
    /// The process main heap was unavailable.
    MainUnavailable,
    /// The requested heap operation is not supported by the safe facade.
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
/// ```rust
/// use core::alloc::Layout;
/// use rimalloc::Heap;
///
/// let heap = Heap::new().unwrap();
/// let heap_ref = heap.as_ref();
///
/// let allocation = heap.allocate(Layout::from_size_align(16, 8).unwrap()).unwrap();
/// assert_eq!(allocation.len(), 16);
///
/// let zeroed = heap_ref.allocate_zeroed_bytes(4).unwrap();
/// assert_eq!(zeroed.as_bytes(), &[0, 0, 0, 0]);
/// ```
///
/// Heap-tied allocations borrow the heap, so safe code cannot move them beyond
/// the heap lifetime:
///
/// ```rust,compile_fail
/// use core::alloc::Layout;
/// use rimalloc::{Heap, HeapAllocation};
///
/// let layout = Layout::from_size_align(16, 8).unwrap();
/// let leaked: HeapAllocation<'static>;
/// {
///     let heap = Heap::new().unwrap();
///     leaked = heap.allocate(layout).unwrap();
/// }
/// drop(leaked);
/// ```
///
/// Initialized heap allocations carry the same heap lifetime:
///
/// ```rust,compile_fail
/// use rimalloc::{Heap, InitializedHeapAllocation};
///
/// let leaked: InitializedHeapAllocation<'static>;
/// {
///     let heap = Heap::new().unwrap();
///     leaked = heap.allocate_zeroed_bytes(16).unwrap();
/// }
/// drop(leaked);
/// ```
///
/// Safe code also cannot move or drop a heap while a heap-tied allocation is
/// still live:
///
/// ```rust,compile_fail
/// use rimalloc::Heap;
///
/// let heap = Heap::new().unwrap();
/// let allocation = heap.allocate_bytes(16).unwrap();
/// drop(heap);
/// drop(allocation);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::Heap;
///
/// let heap = Heap::new().unwrap();
/// let allocation = heap.allocate_zeroed_bytes(16).unwrap();
/// drop(heap);
/// drop(allocation);
/// ```
///
/// Heap handles and heap-tied allocations are intentionally not `Send` or
/// `Sync`. Safe heap allocation, resize, and stats operations use mimalloc
/// heap/thread-heap metadata that is not exposed as a thread-safe Rust handle.
///
/// ```rust,compile_fail
/// use rimalloc::Heap;
///
/// struct MustBeSend<T: Send>(core::marker::PhantomData<T>);
/// let _ = MustBeSend::<Heap>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::HeapRef;
///
/// struct MustBeSend<T: Send>(core::marker::PhantomData<T>);
/// let _ = MustBeSend::<HeapRef<'static>>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::MainHeap;
///
/// struct MustBeSend<T: Send>(core::marker::PhantomData<T>);
/// let _ = MustBeSend::<MainHeap>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::HeapAllocation;
///
/// struct MustBeSend<T: Send>(core::marker::PhantomData<T>);
/// let _ = MustBeSend::<HeapAllocation<'static>>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::InitializedHeapAllocation;
///
/// struct MustBeSend<T: Send>(core::marker::PhantomData<T>);
/// let _ = MustBeSend::<InitializedHeapAllocation<'static>>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::Heap;
///
/// struct MustBeSync<T: Sync>(core::marker::PhantomData<T>);
/// let _ = MustBeSync::<Heap>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::HeapRef;
///
/// struct MustBeSync<T: Sync>(core::marker::PhantomData<T>);
/// let _ = MustBeSync::<HeapRef<'static>>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::MainHeap;
///
/// struct MustBeSync<T: Sync>(core::marker::PhantomData<T>);
/// let _ = MustBeSync::<MainHeap>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::HeapAllocation;
///
/// struct MustBeSync<T: Sync>(core::marker::PhantomData<T>);
/// let _ = MustBeSync::<HeapAllocation<'static>>(core::marker::PhantomData);
/// ```
///
/// ```rust,compile_fail
/// use rimalloc::InitializedHeapAllocation;
///
/// struct MustBeSync<T: Sync>(core::marker::PhantomData<T>);
/// let _ = MustBeSync::<InitializedHeapAllocation<'static>>(core::marker::PhantomData);
/// ```
#[must_use]
pub struct Heap {
    ptr: NonNull<crate::types::mi_heap_t>,
    _not_send_sync: NotSendSync,
}

impl Heap {
    /// Creates a new owned mimalloc heap.
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::Create`] if mimalloc cannot create the heap.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn new() -> Result<Self, HeapError> {
        let ptr = unsafe { crate::allocator::heap::mi_heap_new() };
        let ptr = NonNull::new(ptr).ok_or(HeapError::Create)?;
        Ok(Self {
            ptr,
            _not_send_sync: PhantomData,
        })
    }

    /// Borrows this heap as a copyable [`HeapRef`].
    #[inline]
    pub fn as_ref(&self) -> HeapRef<'_> {
        HeapRef {
            ptr: self.ptr,
            _marker: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    /// Allocates an uninitialized byte block from this heap.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if mimalloc cannot satisfy a non-zero allocation
    /// request for `layout`.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate(&self, layout: Layout) -> Result<HeapAllocation<'_>, AllocError> {
        self.as_ref().allocate(layout)
    }

    /// Allocates an uninitialized byte block with alignment 1 from this heap.
    ///
    /// # Errors
    ///
    /// Returns [`TryAllocError::Layout`] if `len` cannot form a Rust layout, or
    /// [`TryAllocError::Alloc`] if mimalloc cannot satisfy the request.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_bytes(&self, len: usize) -> Result<HeapAllocation<'_>, TryAllocError> {
        self.as_ref().allocate_bytes(len)
    }

    /// Allocates a zero-initialized byte block from this heap.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if mimalloc cannot satisfy a non-zero allocation
    /// request for `layout`.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_zeroed(
        &self,
        layout: Layout,
    ) -> Result<InitializedHeapAllocation<'_>, AllocError> {
        self.as_ref().allocate_zeroed(layout)
    }

    /// Allocates a zero-initialized byte block with alignment 1 from this heap.
    ///
    /// # Errors
    ///
    /// Returns [`TryAllocError::Layout`] if `len` cannot form a Rust layout, or
    /// [`TryAllocError::Alloc`] if mimalloc cannot satisfy the request.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_zeroed_bytes(
        &self,
        len: usize,
    ) -> Result<InitializedHeapAllocation<'_>, TryAllocError> {
        self.as_ref().allocate_zeroed_bytes(len)
    }

    /// Captures a statistics snapshot for this heap.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::HeapUnavailable`] if mimalloc cannot capture heap
    /// statistics for this heap.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn stats(&self) -> Result<StatsSnapshot, StatsError> {
        self.as_ref().stats()
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe { crate::allocator::heap::mi_heap_delete(self.ptr.as_ptr()) };
    }
}

impl fmt::Debug for Heap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Heap")
            .field("ptr", &self.ptr)
            .finish_non_exhaustive()
    }
}

/// Borrowed heap handle used to tie heap allocations to a heap lifetime.
#[derive(Clone, Copy)]
#[must_use]
pub struct HeapRef<'heap> {
    pub(super) ptr: NonNull<crate::types::mi_heap_t>,
    _marker: PhantomData<&'heap Heap>,
    _not_send_sync: NotSendSync,
}

impl<'heap> HeapRef<'heap> {
    /// Allocates an uninitialized byte block from this borrowed heap.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if mimalloc cannot satisfy a non-zero allocation
    /// request for `layout`.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate(self, layout: Layout) -> Result<HeapAllocation<'heap>, AllocError> {
        HeapAllocation::allocate_in(self.ptr, layout, false)
    }

    /// Allocates an uninitialized byte block with alignment 1 from this borrowed heap.
    ///
    /// # Errors
    ///
    /// Returns [`TryAllocError::Layout`] if `len` cannot form a Rust layout, or
    /// [`TryAllocError::Alloc`] if mimalloc cannot satisfy the request.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_bytes(self, len: usize) -> Result<HeapAllocation<'heap>, TryAllocError> {
        self.allocate(try_layout(len, 1)?)
            .map_err(TryAllocError::Alloc)
    }

    /// Allocates a zero-initialized byte block from this borrowed heap.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if mimalloc cannot satisfy a non-zero allocation
    /// request for `layout`.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_zeroed(
        self,
        layout: Layout,
    ) -> Result<InitializedHeapAllocation<'heap>, AllocError> {
        InitializedHeapAllocation::allocate_zeroed_in(self.ptr, layout)
    }

    /// Allocates a zero-initialized byte block with alignment 1 from this borrowed heap.
    ///
    /// # Errors
    ///
    /// Returns [`TryAllocError::Layout`] if `len` cannot form a Rust layout, or
    /// [`TryAllocError::Alloc`] if mimalloc cannot satisfy the request.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_zeroed_bytes(
        self,
        len: usize,
    ) -> Result<InitializedHeapAllocation<'heap>, TryAllocError> {
        self.allocate_zeroed(try_layout(len, 1)?)
            .map_err(TryAllocError::Alloc)
    }

    /// Captures a statistics snapshot for this heap.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::HeapUnavailable`] if mimalloc cannot capture heap
    /// statistics for this heap.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn stats(self) -> Result<StatsSnapshot, StatsError> {
        StatsSnapshot::for_heap(self)
    }
}

impl fmt::Debug for HeapRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeapRef")
            .field("ptr", &self.ptr)
            .finish_non_exhaustive()
    }
}

/// Borrowed process main heap handle.
///
/// `MainHeap` does not own or delete the process main heap. Allocations made
/// through it use the same heap-tied allocation wrappers as owned heaps, but
/// are tied to the process-global main heap lifetime instead of to a temporary
/// `MainHeap` value.
///
/// ```rust
/// use rimalloc::MainHeap;
///
/// let main = MainHeap::get().unwrap();
/// let main_ref = main.as_ref();
///
/// let allocation = main.allocate_zeroed_bytes(8).unwrap();
/// assert_eq!(allocation.as_bytes(), &[0; 8]);
///
/// let stats = main_ref.stats().unwrap();
/// assert!(stats.reserved_peak().get() >= stats.reserved_current().get());
/// ```
///
/// `MainHeap` allocations use the process-global main heap lifetime, so they
/// can outlive the temporary handle value:
///
/// ```rust
/// use rimalloc::MainHeap;
///
/// let allocation = {
///     let main = MainHeap::get().unwrap();
///     main.allocate_bytes(8).unwrap()
/// };
/// assert_eq!(allocation.len(), 8);
///
/// let zeroed = MainHeap::get().unwrap().allocate_zeroed_bytes(4).unwrap();
/// assert_eq!(zeroed.as_bytes(), &[0, 0, 0, 0]);
/// ```
#[derive(Clone, Copy)]
#[must_use]
pub struct MainHeap {
    ptr: NonNull<crate::types::mi_heap_t>,
    _not_send_sync: NotSendSync,
}

impl fmt::Debug for MainHeap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MainHeap")
            .field("ptr", &self.ptr)
            .finish_non_exhaustive()
    }
}

impl MainHeap {
    /// Returns a borrowed handle to the process main heap.
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::MainUnavailable`] if mimalloc does not provide the
    /// process main heap.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn get() -> Result<Self, HeapError> {
        let ptr = crate::allocator::init::mi_heap_main();
        let ptr = NonNull::new(ptr).ok_or(HeapError::MainUnavailable)?;
        Ok(Self {
            ptr,
            _not_send_sync: PhantomData,
        })
    }

    /// Borrows the process-global main heap as a copyable [`HeapRef`].
    ///
    /// This returns `HeapRef<'static>` because `MainHeap` is only a borrowed
    /// witness for mimalloc's process main heap. Dropping `MainHeap` does not
    /// delete the heap, so allocations made through this reference are not tied
    /// to the short borrow of the handle value.
    #[inline]
    pub fn as_ref(&self) -> HeapRef<'static> {
        HeapRef {
            ptr: self.ptr,
            _marker: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    /// Allocates an uninitialized byte block from the process main heap.
    ///
    /// The returned allocation is tied to the process-global main heap, not to
    /// this temporary `MainHeap` handle.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if mimalloc cannot satisfy a non-zero allocation
    /// request for `layout`.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate(&self, layout: Layout) -> Result<HeapAllocation<'static>, AllocError> {
        self.as_ref().allocate(layout)
    }

    /// Allocates an uninitialized byte block with alignment 1 from the process main heap.
    ///
    /// The returned allocation is tied to the process-global main heap, not to
    /// this temporary `MainHeap` handle.
    ///
    /// # Errors
    ///
    /// Returns [`TryAllocError::Layout`] if `len` cannot form a Rust layout, or
    /// [`TryAllocError::Alloc`] if mimalloc cannot satisfy the request.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_bytes(&self, len: usize) -> Result<HeapAllocation<'static>, TryAllocError> {
        self.as_ref().allocate_bytes(len)
    }

    /// Allocates a zero-initialized byte block from the process main heap.
    ///
    /// The returned allocation is tied to the process-global main heap, not to
    /// this temporary `MainHeap` handle.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if mimalloc cannot satisfy a non-zero allocation
    /// request for `layout`.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_zeroed(
        &self,
        layout: Layout,
    ) -> Result<InitializedHeapAllocation<'static>, AllocError> {
        self.as_ref().allocate_zeroed(layout)
    }

    /// Allocates a zero-initialized byte block with alignment 1 from the process main heap.
    ///
    /// The returned allocation is tied to the process-global main heap, not to
    /// this temporary `MainHeap` handle.
    ///
    /// # Errors
    ///
    /// Returns [`TryAllocError::Layout`] if `len` cannot form a Rust layout, or
    /// [`TryAllocError::Alloc`] if mimalloc cannot satisfy the request.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn allocate_zeroed_bytes(
        &self,
        len: usize,
    ) -> Result<InitializedHeapAllocation<'static>, TryAllocError> {
        self.as_ref().allocate_zeroed_bytes(len)
    }

    /// Captures a statistics snapshot for the main heap.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::HeapUnavailable`] if mimalloc cannot capture heap
    /// statistics for the process main heap.
    ///
    /// # Panics
    ///
    /// Does not panic.
    #[inline]
    pub fn stats(&self) -> Result<StatsSnapshot, StatsError> {
        self.as_ref().stats()
    }
}

/// Allocation tied to the lifetime of the heap borrow that created it.
///
/// ```rust
/// use core::alloc::Layout;
/// use rimalloc::Heap;
///
/// let heap = Heap::new().unwrap();
/// let mut allocation = heap.allocate(Layout::from_size_align(3, 1).unwrap()).unwrap();
/// for (slot, byte) in allocation.as_uninit_slice_mut().iter_mut().zip([7, 8, 9]) {
///     slot.write(byte);
/// }
/// let initialized = unsafe { allocation.assume_initialized() };
/// assert_eq!(initialized.as_bytes(), &[7, 8, 9]);
/// ```
///
/// ```rust,compile_fail
/// use core::alloc::Layout;
/// use rimalloc::{Heap, HeapAllocation};
///
/// fn allocate_then_drop_heap() -> HeapAllocation<'static> {
///     let heap = Heap::new().unwrap();
///     heap.allocate(Layout::from_size_align(32, 8).unwrap()).unwrap()
/// }
/// ```
///
/// External code cannot construct a heap allocation from representation fields:
///
/// ```rust,compile_fail
/// let _ = rimalloc::HeapAllocation { raw: unreachable!() };
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
    _not_send_sync: NotSendSync,
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

    /// Returns the layout used to allocate this heap block.
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

    /// Returns a shared raw pointer to the heap allocation.
    #[inline]
    #[must_use]
    pub const fn as_ptr(&self) -> *const u8 {
        self.raw.ptr.as_ptr().cast_const()
    }

    /// Returns a mutable raw pointer to the heap allocation.
    #[inline]
    #[must_use]
    pub const fn as_mut_ptr(&mut self) -> *mut u8 {
        self.raw.ptr.as_ptr()
    }

    /// Returns this uninitialized heap allocation as a `MaybeUninit<u8>` slice.
    ///
    /// The slice length is the requested allocation size. Zero-sized heap
    /// allocations return an empty slice and do not dereference the sentinel
    /// pointer.
    #[inline]
    #[must_use]
    pub fn as_uninit_slice(&self) -> &[MaybeUninit<u8>] {
        unsafe { core::slice::from_raw_parts(self.as_ptr().cast(), self.len()) }
    }

    /// Returns this uninitialized heap allocation as a mutable
    /// `MaybeUninit<u8>` slice.
    ///
    /// Use this to initialize bytes without raw pointer loops, then keep the
    /// transition to initialized reads explicit.
    #[inline]
    #[must_use]
    pub fn as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr().cast(), self.len()) }
    }

    /// Resizes this heap allocation to `new_layout`.
    ///
    /// # Errors
    ///
    /// Returns [`ResizeError`] if mimalloc cannot satisfy a non-zero resize
    /// request. The error preserves the original allocation.
    ///
    /// # Panics
    ///
    /// Does not panic.
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
    ///
    /// # Errors
    ///
    /// Returns [`ResizeError`] with [`ResizeErrorKind::Layout`] if `size` and
    /// `align` do not form a valid [`Layout`], or [`ResizeErrorKind::Alloc`] if
    /// mimalloc cannot satisfy the resize request. The error preserves the
    /// original allocation.
    ///
    /// # Panics
    ///
    /// Does not panic.
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

    /// Marks this heap allocation initialized after the caller has initialized
    /// every byte through raw pointers or `MaybeUninit` views.
    ///
    /// # Safety
    ///
    /// The caller must guarantee every byte in the allocation has been
    /// initialized before calling this method.
    #[inline]
    pub unsafe fn assume_initialized(self) -> InitializedHeapAllocation<'heap> {
        InitializedHeapAllocation { raw: self.raw }
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
                _not_send_sync: PhantomData,
            });
        }

        #[cfg(test)]
        if super::failure_injection::should_fail_allocation() {
            return Err(AllocError { layout });
        }

        let ptr = unsafe {
            if crate::allocator::aligned::mi_malloc_satisfies_alignment(
                layout.size(),
                layout.align(),
            ) {
                if zeroed {
                    crate::allocator::alloc::mi_heap_zalloc(heap.as_ptr(), layout.size())
                } else {
                    crate::allocator::alloc::mi_heap_malloc(heap.as_ptr(), layout.size())
                }
            } else if zeroed {
                crate::allocator::aligned::mi_heap_zalloc_aligned(
                    heap.as_ptr(),
                    layout.size(),
                    layout.align(),
                )
            } else {
                crate::allocator::aligned::mi_heap_malloc_aligned(
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
            _not_send_sync: PhantomData,
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
                unsafe { crate::ffi::mimalloc::mi_free(old.ptr.as_ptr().cast::<c_void>()) };
            }
            return Ok(Self {
                ptr: dangling_for(new_layout),
                layout: new_layout,
                heap: old.heap,
                _marker: PhantomData,
                _not_send_sync: PhantomData,
            });
        }

        if old_size != 0 && old_layout.align() == new_layout.align() {
            #[cfg(test)]
            if super::failure_injection::should_fail_allocation() {
                return Err(ResizeError {
                    allocation: ManuallyDrop::into_inner(old),
                    kind: ResizeErrorKind::Alloc(AllocError { layout: new_layout }),
                });
            }

            let ptr = unsafe {
                if crate::allocator::aligned::mi_malloc_satisfies_alignment(
                    new_size,
                    old_layout.align(),
                ) {
                    crate::allocator::alloc::mi_heap_realloc(
                        old.heap.as_ptr(),
                        old.ptr.as_ptr().cast::<c_void>(),
                        new_size,
                    )
                } else {
                    crate::allocator::aligned::mi_heap_realloc_aligned(
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
                _not_send_sync: PhantomData,
            };
            if zero_grown_tail && new_size > old_size {
                allocation.zero_tail(old_size);
            }
            return Ok(allocation);
        }

        let zero_all = zero_grown_tail && old_size == 0;
        let new_allocation = match Self::allocate_in(old.heap, new_layout, zero_all) {
            Ok(allocation) => allocation,
            Err(err) => {
                return Err(ResizeError {
                    allocation: ManuallyDrop::into_inner(old),
                    kind: ResizeErrorKind::Alloc(err),
                });
            }
        };
        if old_size != 0 {
            unsafe {
                ptr::copy_nonoverlapping(
                    old.ptr.as_ptr(),
                    new_allocation.ptr.as_ptr(),
                    old_size.min(new_size),
                );
                crate::ffi::mimalloc::mi_free(old.ptr.as_ptr().cast::<c_void>());
            }
        }
        if zero_grown_tail && !zero_all && new_size > old_size {
            new_allocation.zero_tail(old_size);
        }
        Ok(new_allocation)
    }

    #[inline]
    fn zero_tail(&self, initialized_prefix: usize) {
        let len = self.layout.size();
        if initialized_prefix < len {
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
            unsafe { crate::ffi::mimalloc::mi_free(self.ptr.as_ptr().cast::<c_void>()) };
        }
    }
}

impl fmt::Debug for RawHeapAllocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawHeapAllocation")
            .field("ptr", &self.ptr)
            .field("layout", &self.layout)
            .field("heap", &self.heap)
            .finish_non_exhaustive()
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
///
/// ```rust
/// use rimalloc::Heap;
///
/// let heap = Heap::new().unwrap();
/// let mut allocation = heap.allocate_zeroed_bytes(4).unwrap();
/// allocation.as_mut_bytes().copy_from_slice(&[4, 3, 2, 1]);
///
/// let uninitialized = allocation.into_allocation();
/// assert_eq!(uninitialized.len(), 4);
/// ```
///
/// External code cannot construct an initialized heap allocation from
/// representation fields:
///
/// ```rust,compile_fail
/// let _ = rimalloc::InitializedHeapAllocation { raw: unreachable!() };
/// ```
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

    /// Returns the layout used to allocate this heap block.
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
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
    }

    /// Returns the initialized bytes as a mutable slice.
    #[inline]
    #[must_use]
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.len()) }
    }

    /// Converts this initialized wrapper back into a heap allocation wrapper.
    #[inline]
    pub fn into_allocation(self) -> HeapAllocation<'heap> {
        HeapAllocation { raw: self.raw }
    }

    /// Resizes this initialized heap allocation, zeroing any newly exposed tail.
    ///
    /// # Errors
    ///
    /// Returns [`ResizeError`] if mimalloc cannot satisfy a non-zero resize
    /// request. The error preserves the original allocation.
    ///
    /// # Panics
    ///
    /// Does not panic.
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
    ///
    /// # Errors
    ///
    /// Returns [`ResizeError`] with [`ResizeErrorKind::Layout`] if `size` and
    /// `align` do not form a valid [`Layout`], or [`ResizeErrorKind::Alloc`] if
    /// mimalloc cannot satisfy the resize request. The error preserves the
    /// original allocation.
    ///
    /// # Panics
    ///
    /// Does not panic.
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
}

impl fmt::Debug for InitializedHeapAllocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InitializedHeapAllocation")
            .field("raw", &self.raw)
            .finish()
    }
}

impl AsRef<[u8]> for InitializedHeapAllocation<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsMut<[u8]> for InitializedHeapAllocation<'_> {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_bytes()
    }
}
