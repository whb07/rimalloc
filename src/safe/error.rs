use core::alloc::Layout;
use core::fmt;

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
    pub(super) layout: Layout,
}

impl AllocError {
    /// Returns the layout for the failed allocation request.
    #[inline]
    #[must_use]
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
/// let err = match Allocation::with_size_align(8, 3) {
///     Err(err) => err,
///     Ok(_) => unreachable!("invalid alignment should not allocate"),
/// };
/// match err {
///     TryAllocError::Layout(layout) => {
///         assert_eq!(layout.size(), 8);
///         assert_eq!(layout.align(), 3);
///     }
///     TryAllocError::Alloc(_) => unreachable!(),
///     _ => unreachable!(),
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutError {
    pub(super) size: usize,
    pub(super) align: usize,
}

impl LayoutError {
    /// Returns the rejected size.
    #[inline]
    #[must_use]
    pub const fn size(self) -> usize {
        self.size
    }

    /// Returns the rejected alignment.
    #[inline]
    #[must_use]
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
/// let err = match Allocation::zeroed_with_size_align(1, 3) {
///     Err(err) => err,
///     Ok(_) => unreachable!("invalid alignment should not allocate"),
/// };
/// let label = match err {
///     TryAllocError::Layout(_) => "layout",
///     TryAllocError::Alloc(_) => "allocation",
///     _ => "future",
/// };
/// assert_eq!(label, "layout");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TryAllocError {
    /// The requested size/alignment pair was not a valid [`Layout`].
    Layout(LayoutError),
    /// The layout was valid, but mimalloc returned null.
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
impl std::error::Error for TryAllocError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(err) => Some(err),
            Self::Alloc(err) => Some(err),
        }
    }
}

/// Cause of a failed resize operation.
///
/// ```rust
/// use mechanical_mimalloc::{Allocation, ResizeErrorKind};
///
/// let allocation = Allocation::new_bytes(8).unwrap();
/// let err = allocation.resize_with_size_align(8, 3).unwrap_err();
/// assert!(matches!(err.kind(), ResizeErrorKind::Layout(_)));
/// assert_eq!(err.allocation().len(), 8);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResizeErrorKind {
    /// The requested new size/alignment pair was not a valid [`Layout`].
    Layout(LayoutError),
    /// The requested new layout was valid, but mimalloc returned null.
    Alloc(AllocError),
}

impl fmt::Display for ResizeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(err) => err.fmt(f),
            Self::Alloc(err) => err.fmt(f),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ResizeErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(err) => Some(err),
            Self::Alloc(err) => Some(err),
        }
    }
}

/// Error returned when resizing fails without consuming the original allocation.
///
/// ```rust
/// use mechanical_mimalloc::{Allocation, ResizeErrorKind};
///
/// let allocation = Allocation::zeroed_bytes(4).unwrap();
/// let err = allocation.resize_with_size_align(4, 3).unwrap_err();
/// let (allocation, kind) = err.into_parts();
/// assert!(matches!(kind, ResizeErrorKind::Layout(_)));
/// assert_eq!(allocation.len(), 4);
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct ResizeError<A> {
    pub(super) allocation: A,
    pub(super) kind: ResizeErrorKind,
}

impl<A> ResizeError<A> {
    /// Returns the resize failure kind.
    #[inline]
    #[must_use]
    pub const fn kind(&self) -> ResizeErrorKind {
        self.kind
    }

    /// Returns the original allocation preserved after resize failure.
    #[inline]
    #[must_use]
    pub const fn allocation(&self) -> &A {
        &self.allocation
    }

    /// Returns mutable access to the original allocation.
    #[inline]
    #[must_use]
    pub const fn allocation_mut(&mut self) -> &mut A {
        &mut self.allocation
    }

    /// Consumes the error and returns the original allocation.
    #[inline]
    #[must_use]
    pub fn into_allocation(self) -> A {
        self.allocation
    }

    /// Consumes the error and returns the original allocation and failure kind.
    #[inline]
    #[must_use]
    pub fn into_parts(self) -> (A, ResizeErrorKind) {
        (self.allocation, self.kind)
    }
}

impl<A> fmt::Display for ResizeError<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

#[cfg(feature = "std")]
impl<A: fmt::Debug> std::error::Error for ResizeError<A> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}
