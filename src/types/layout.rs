//! DT-3 allocation layout types.

use super::ptr::{PagePtr, UserPtr};
use super::scalar::{
    Alignment, AlignmentOffset, EffectiveSize, MaybeAlignment, PaddedSize, RequestedSize,
    UsableSize,
};

/// Represents an explicit zeroing request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Zeroed {
    No,
    Yes,
}

impl Zeroed {
    #[inline(always)]
    pub(crate) const fn from_bool(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }

    #[inline(always)]
    pub(crate) const fn as_bool(self) -> bool {
        matches!(self, Self::Yes)
    }
}

/// Represents the optional huge-page alignment payload packed into the generic
/// allocation flag word. The raw value must keep bit 0 clear; bit 0 is reserved
/// for the zeroing flag.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct MaybeHugeAlignment(usize);

impl MaybeHugeAlignment {
    #[inline(always)]
    pub(crate) const fn none() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub(crate) fn new(alignment: usize) -> Self {
        crate::internal::mi_debug_assert!(
            (alignment & MallocGenericFlags::ZERO_BIT) == 0,
            "huge alignment must not collide with the zeroing flag"
        );
        Self(alignment)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Internal errors when creating allocator layout structures.
///
/// This is intentionally separate from the public safe facade's
/// `crate::safe::LayoutError`: internal allocation paths need compact enum
/// reasons, while the safe API reports the rejected size/alignment pair.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LayoutError {
    SizeOverflow,
    AlignmentInvalid,
    AlignmentOverflow,
}

/// Standard verified allocation layout request.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LayoutRequest {
    requested: RequestedSize,
    effective: EffectiveSize,
    padded: PaddedSize,
    alignment: MaybeAlignment,
    zeroed: Zeroed,
}

impl LayoutRequest {
    fn from_parts(
        requested: RequestedSize,
        effective: EffectiveSize,
        alignment: MaybeAlignment,
        zeroed: Zeroed,
    ) -> Result<Self, LayoutError> {
        let padded = RequestedSize::new(effective.get())
            .checked_padded()
            .map_err(|_| LayoutError::SizeOverflow)?;
        Ok(Self {
            requested,
            effective,
            padded,
            alignment,
            zeroed,
        })
    }

    pub(crate) fn malloc(size: usize) -> Result<Self, LayoutError> {
        let requested = RequestedSize::new(size);
        // Mimalloc normalizes zero-size C allocation requests to the minimum
        // word-sized allocation before choosing a size class. Keep the original
        // requested size for API-visible accounting, but never build a zero
        // `PaddedSize`.
        let eff_size = requested.effective_size();
        let effective = unsafe { EffectiveSize::new_unchecked(eff_size) };
        Self::from_parts(
            requested,
            effective,
            unsafe { MaybeAlignment::new_unchecked(0) },
            Zeroed::No,
        )
    }

    pub(crate) fn zalloc(size: usize) -> Result<Self, LayoutError> {
        let mut req = Self::malloc(size)?;
        req.zeroed = Zeroed::Yes;
        Ok(req)
    }

    fn maybe_alignment_from_raw(alignment: usize) -> Result<MaybeAlignment, LayoutError> {
        if alignment == 0 {
            return Ok(unsafe { MaybeAlignment::new_unchecked(0) });
        }
        let alignment = Alignment::new(alignment).ok_or(LayoutError::AlignmentInvalid)?;
        Ok(unsafe { MaybeAlignment::new_unchecked(alignment.get()) })
    }

    pub(crate) fn aligned(size: usize, alignment: usize) -> Result<Self, LayoutError> {
        let alignment = Self::maybe_alignment_from_raw(alignment)?;
        let mut req = Self::malloc(size)?;
        req.alignment = alignment;
        Ok(req)
    }

    pub(crate) fn aligned_at(
        size: usize,
        alignment: usize,
        offset: AlignmentOffset,
        zeroed: Zeroed,
    ) -> Result<Self, LayoutError> {
        let alignment = Self::maybe_alignment_from_raw(alignment)?;
        // Basic overflow check for the `aligned_at` calculation
        if size.checked_add(offset.get()).is_none() || size.checked_add(alignment.get()).is_none() {
            return Err(LayoutError::AlignmentOverflow);
        }

        let mut req = Self::malloc(size)?;
        req.alignment = alignment;
        req.zeroed = zeroed;
        Ok(req)
    }

    /// Construct a layout from a size that already includes mimalloc padding.
    ///
    /// # Safety
    /// `padded` must be the result of adding `MI_PADDING_SIZE` to the logical
    /// requested size for this allocation path.
    pub(crate) unsafe fn from_padded_unchecked(padded: PaddedSize, zeroed: Zeroed) -> Self {
        crate::internal::mi_debug_assert!(
            padded.get() >= crate::types::MI_PADDING_SIZE,
            "padded allocation size must include padding"
        );
        let requested_size = padded.get().wrapping_sub(crate::types::MI_PADDING_SIZE);
        let effective_size = RequestedSize::new(requested_size).effective_size();
        Self {
            requested: RequestedSize::new(requested_size),
            effective: unsafe { EffectiveSize::new_unchecked(effective_size) },
            padded,
            alignment: unsafe { MaybeAlignment::new_unchecked(0) },
            zeroed,
        }
    }
}

/// Represents the number of items for `calloc`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ItemCount(pub(crate) usize);

impl ItemCount {
    pub(crate) fn new(count: usize) -> Self {
        Self(count)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CallocRequest {
    total: RequestedSize,
    layout: LayoutRequest,
}

impl CallocRequest {
    pub(crate) fn new(count: usize, size: usize) -> Result<Self, LayoutError> {
        let total_size = count.checked_mul(size).ok_or(LayoutError::SizeOverflow)?;
        Self::from_checked_total(total_size)
    }

    /// Builds a zeroing request for a precomputed total size.
    #[inline]
    pub(crate) fn from_checked_total(total_size: usize) -> Result<Self, LayoutError> {
        let layout = LayoutRequest::zalloc(total_size)?;
        Ok(Self {
            total: RequestedSize::new(total_size),
            layout,
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct AllocProduct {
    total: RequestedSize,
    layout: LayoutRequest,
}

impl AllocProduct {
    /// Builds a non-zeroing request for a precomputed total size.
    #[inline]
    pub(crate) fn from_checked_total(total_size: usize) -> Result<Self, LayoutError> {
        let layout = LayoutRequest::malloc(total_size)?;
        Ok(Self {
            total: RequestedSize::new(total_size),
            layout,
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OldAllocation {
    ptr: UserPtr,
    usable: UsableSize,
    page: PagePtr,
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PreserveBytes(usize);

impl PreserveBytes {
    pub(crate) fn new(old: UsableSize, new: RequestedSize) -> Self {
        Self(usize::min(old.get(), new.get()))
    }

    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

impl OldAllocation {
    unsafe fn new(ptr: UserPtr, usable: UsableSize, page: PagePtr) -> Self {
        Self { ptr, usable, page }
    }

    /// Constructs an old allocation after pointer validation has proven the
    /// user pointer and page belong together.
    ///
    /// # Safety
    /// `ptr` and `page` must come from the same successful
    /// `PointerLookup::Valid` result.
    pub(crate) unsafe fn from_validated_parts(
        ptr: UserPtr,
        usable: UsableSize,
        page: PagePtr,
    ) -> Self {
        unsafe { Self::new(ptr, usable, page) }
    }

    pub(crate) fn ptr(self) -> UserPtr {
        self.ptr
    }

    pub(crate) fn usable(self) -> UsableSize {
        self.usable
    }

    pub(crate) fn page(self) -> PagePtr {
        self.page
    }
}

#[derive(Clone, Copy)]
enum ReallocSource {
    Null,
    Existing(OldAllocation),
}

pub(crate) struct ReallocRequest {
    source: ReallocSource,
    new_layout: LayoutRequest,
    zero_new_tail: Zeroed,
}

impl ReallocRequest {
    pub(crate) fn from_null(new_layout: LayoutRequest, zero_new_tail: Zeroed) -> Self {
        Self {
            source: ReallocSource::Null,
            new_layout,
            zero_new_tail,
        }
    }

    pub(crate) fn from_existing(
        old: OldAllocation,
        new_layout: LayoutRequest,
        zero_new_tail: Zeroed,
    ) -> Self {
        Self {
            source: ReallocSource::Existing(old),
            new_layout,
            zero_new_tail,
        }
    }

    pub(crate) fn ptr(&self) -> Option<UserPtr> {
        match self.source {
            ReallocSource::Null => None,
            ReallocSource::Existing(old) => Some(old.ptr()),
        }
    }

    pub(crate) fn old(&self) -> Option<OldAllocation> {
        match self.source {
            ReallocSource::Null => None,
            ReallocSource::Existing(old) => Some(old),
        }
    }

    pub(crate) fn new_layout(&self) -> LayoutRequest {
        self.new_layout
    }

    pub(crate) fn zero_new_tail(&self) -> Zeroed {
        self.zero_new_tail
    }
}

/// Generic malloc flags enforcing mutual exclusion of alignment bits and zeroing bit.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct MallocGenericFlags(usize);

impl MallocGenericFlags {
    const ZERO_BIT: usize = 1;

    #[inline(always)]
    pub(crate) fn new(zeroed: Zeroed, huge_alignment: MaybeHugeAlignment) -> Self {
        let z = if zeroed == Zeroed::Yes {
            Self::ZERO_BIT
        } else {
            0
        };
        crate::internal::mi_debug_assert!(
            (huge_alignment.get() & Self::ZERO_BIT) == 0,
            "huge alignment must not collide with the zeroing flag"
        );
        Self(z | huge_alignment.get())
    }

    /// Decode the raw C-compatible flag word. Bit 0 selects zeroing and every
    /// remaining bit is retained as the huge-alignment payload; consumers
    /// validate that payload before using it as an alignment.
    #[inline(always)]
    pub(crate) fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub(crate) fn zeroed(self) -> Zeroed {
        if (self.0 & Self::ZERO_BIT) != 0 {
            Zeroed::Yes
        } else {
            Zeroed::No
        }
    }

    #[inline(always)]
    pub(crate) fn huge_alignment(self) -> MaybeHugeAlignment {
        MaybeHugeAlignment(self.0 & !Self::ZERO_BIT)
    }
}
impl LayoutRequest {
    pub(crate) fn requested(self) -> RequestedSize {
        self.requested
    }
    /// The padded size (`requested + MI_PADDING_SIZE`), computed once at
    /// construction via `RequestedSize::checked_padded`. Lower layers that need
    /// the padded size should read this instead of re-adding `MI_PADDING_SIZE`.
    pub(crate) fn padded(self) -> PaddedSize {
        self.padded
    }
    /// The effective size after mimalloc's zero-size normalization (never zero).
    pub(crate) fn effective(self) -> EffectiveSize {
        self.effective
    }
    pub(crate) fn zeroed(self) -> Zeroed {
        self.zeroed
    }
    pub(crate) fn alignment(self) -> MaybeAlignment {
        self.alignment
    }
}

impl CallocRequest {
    pub(crate) fn total(self) -> RequestedSize {
        self.total
    }
    pub(crate) fn layout(self) -> LayoutRequest {
        self.layout
    }
}

impl AllocProduct {
    pub(crate) fn total(self) -> RequestedSize {
        self.total
    }
    pub(crate) fn layout(self) -> LayoutRequest {
        self.layout
    }
}

#[cfg(test)]
mod tests;
