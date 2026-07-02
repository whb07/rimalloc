//! DT-3 allocation layout types.

use super::ptr::{BlockPtr, PagePtr, UserPtr};
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
        // SAFETY[AUDIT:typed-layout-domain]: `RequestedSize::effective_size`
        // normalizes zero to the allocator minimum, so this effective size is
        // nonzero.
        let effective = unsafe { EffectiveSize::new_unchecked(eff_size) };
        Self::from_parts(
            requested,
            effective,
            // SAFETY[AUDIT:typed-layout-domain]: zero is the valid sentinel
            // for "no explicit alignment" in `MaybeAlignment`.
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
            // SAFETY[AUDIT:typed-layout-domain]: zero is the valid sentinel
            // for "no explicit alignment" in `MaybeAlignment`.
            return Ok(unsafe { MaybeAlignment::new_unchecked(0) });
        }
        let alignment = Alignment::new(alignment).ok_or(LayoutError::AlignmentInvalid)?;
        // SAFETY[AUDIT:typed-layout-domain]: `Alignment::new` accepted this
        // nonzero power-of-two value, which is a valid `MaybeAlignment`.
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

    /// Construct a layout from a size that has already been normalized to a
    /// nonzero effective allocation size.
    ///
    /// # Safety
    /// `effective.get() + MI_PADDING_SIZE` must not overflow, and the caller
    /// must not need to preserve a distinct zero-size requested value.
    #[inline(always)]
    // SAFETY[AUDIT:typed-layout-domain]: callers provide a nonzero
    // `EffectiveSize` whose padding addition is valid for the allocation path.
    pub(crate) unsafe fn from_effective_unchecked(
        effective: EffectiveSize,
        zeroed: Zeroed,
    ) -> Self {
        let requested = RequestedSize::new(effective.get());
        Self {
            requested,
            effective,
            padded: requested.padded_unchecked(),
            // SAFETY[AUDIT:typed-layout-domain]: zero is the valid sentinel
            // for "no explicit alignment" in `MaybeAlignment`.
            alignment: unsafe { MaybeAlignment::new_unchecked(0) },
            zeroed,
        }
    }

    /// Construct a layout from a size that already includes mimalloc padding.
    ///
    /// # Safety
    /// `padded` must be the result of adding `MI_PADDING_SIZE` to the logical
    /// requested size for this allocation path.
    // SAFETY[AUDIT:typed-layout-domain]: callers provide a padded allocator
    // size whose logical requested component can be recovered by subtracting
    // `MI_PADDING_SIZE`.
    pub(crate) unsafe fn from_padded_unchecked(padded: PaddedSize, zeroed: Zeroed) -> Self {
        crate::internal::mi_debug_assert!(
            padded.get() >= crate::types::MI_PADDING_SIZE,
            "padded allocation size must include padding"
        );
        let requested_size = padded.get().wrapping_sub(crate::types::MI_PADDING_SIZE);
        let effective_size = RequestedSize::new(requested_size).effective_size();
        Self {
            requested: RequestedSize::new(requested_size),
            // SAFETY[AUDIT:typed-layout-domain]: effective-size normalization
            // always returns the allocator's nonzero minimum or the nonzero
            // requested size.
            effective: unsafe { EffectiveSize::new_unchecked(effective_size) },
            padded,
            // SAFETY[AUDIT:typed-layout-domain]: zero is the valid sentinel
            // for "no explicit alignment" in `MaybeAlignment`.
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
    count: ItemCount,
    item_size: RequestedSize,
    total: RequestedSize,
    layout: LayoutRequest,
}

impl CallocRequest {
    pub(crate) fn new(count: usize, size: usize) -> Result<Self, LayoutError> {
        let total_size = count.checked_mul(size).ok_or(LayoutError::SizeOverflow)?;
        // SAFETY[AUDIT:typed-layout-domain]: `total_size` was produced by
        // `count.checked_mul(size)`.
        unsafe { Self::from_checked_total(count, size, total_size) }
    }

    /// Builds a request from a multiplication result already checked by the
    /// caller.
    ///
    /// # Safety
    /// `total_size` must be exactly `count * size` and that multiplication
    /// must not have overflowed.
    #[inline]
    // SAFETY[AUDIT:typed-layout-domain]: callers provide the exact checked
    // multiplication result that this request stores and forwards to zalloc.
    pub(crate) unsafe fn from_checked_total(
        count: usize,
        size: usize,
        total_size: usize,
    ) -> Result<Self, LayoutError> {
        let layout = LayoutRequest::zalloc(total_size)?;
        Ok(Self {
            count: ItemCount::new(count),
            item_size: RequestedSize::new(size),
            total: RequestedSize::new(total_size),
            layout,
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct AllocProduct {
    count: ItemCount,
    item_size: RequestedSize,
    total: RequestedSize,
    layout: LayoutRequest,
}

impl AllocProduct {
    pub(crate) fn new(count: usize, size: usize) -> Result<Self, LayoutError> {
        let total_size = count.checked_mul(size).ok_or(LayoutError::SizeOverflow)?;
        // SAFETY[AUDIT:typed-layout-domain]: `total_size` was produced by
        // `count.checked_mul(size)`.
        unsafe { Self::from_checked_total(count, size, total_size) }
    }

    /// Builds a request from a multiplication result already checked by the
    /// caller.
    ///
    /// # Safety
    /// `total_size` must be exactly `count * size` and that multiplication
    /// must not have overflowed.
    #[inline]
    // SAFETY[AUDIT:typed-layout-domain]: callers provide the exact checked
    // multiplication result that this request stores and forwards to malloc.
    pub(crate) unsafe fn from_checked_total(
        count: usize,
        size: usize,
        total_size: usize,
    ) -> Result<Self, LayoutError> {
        let layout = LayoutRequest::malloc(total_size)?;
        Ok(Self {
            count: ItemCount::new(count),
            item_size: RequestedSize::new(size),
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
    block: BlockPtr,
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
    // SAFETY[AUDIT:typed-layout-domain]: callers have already proven the user
    // pointer, usable size, page, and block describe one allocation.
    unsafe fn new(ptr: UserPtr, usable: UsableSize, page: PagePtr, block: BlockPtr) -> Self {
        Self {
            ptr,
            usable,
            page,
            block,
        }
    }

    /// Constructs an old allocation after pointer validation has proven the
    /// user pointer, page, and block belong together.
    ///
    /// # Safety
    /// `ptr`, `page`, and `block` must come from the same successful
    /// `PointerLookup::Valid` result.
    pub(crate) unsafe fn from_validated_parts(
        ptr: UserPtr,
        usable: UsableSize,
        page: PagePtr,
        block: BlockPtr,
    ) -> Self {
        // SAFETY[AUDIT:typed-layout-domain]: this public unsafe constructor
        // carries the same validated allocation proof into the private packer.
        unsafe { Self::new(ptr, usable, page, block) }
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

    pub(crate) fn block(self) -> BlockPtr {
        self.block
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

    /// # Safety
    ///
    /// Bit 0 must only encode the zeroing flag, and all high bits must be zero
    /// or a valid huge-alignment payload with bit 0 clear.
    #[inline(always)]
    // SAFETY[AUDIT:typed-layout-domain]: callers provide a raw flag word with
    // bit 0 reserved for zeroing and the remaining bits reserved for huge
    // alignment.
    pub(crate) unsafe fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub(crate) const fn raw(self) -> usize {
        self.0
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
    pub(crate) fn count(self) -> ItemCount {
        self.count
    }
    pub(crate) fn item_size(self) -> RequestedSize {
        self.item_size
    }
    pub(crate) fn total(self) -> RequestedSize {
        self.total
    }
    pub(crate) fn layout(self) -> LayoutRequest {
        self.layout
    }
}

impl AllocProduct {
    pub(crate) fn count(self) -> ItemCount {
        self.count
    }
    pub(crate) fn item_size(self) -> RequestedSize {
        self.item_size
    }
    pub(crate) fn total(self) -> RequestedSize {
        self.total
    }
    pub(crate) fn layout(self) -> LayoutRequest {
        self.layout
    }
}

pub(super) fn __lint_touch() {
    let _from_effective: unsafe fn(EffectiveSize, Zeroed) -> LayoutRequest =
        LayoutRequest::from_effective_unchecked;
    if let Ok(req) = CallocRequest::new(1, 1) {
        let _ = req.count();
        let _ = req.item_size();
    }
    if let Ok(req) = AllocProduct::new(1, 1) {
        let _ = req.count();
        let _ = req.item_size();
    }
    let _ = ItemCount::new(1).get();
    // SAFETY[AUDIT:typed-layout-test-fixtures]: these non-dereferenced
    // dangling pointers are lint-touch identity witnesses and are not exposed
    // to allocator state.
    let old = unsafe {
        OldAllocation::from_validated_parts(
            UserPtr::new_unchecked(core::ptr::dangling_mut()),
            UsableSize::new(1),
            PagePtr::new_unchecked(core::ptr::dangling_mut()),
            BlockPtr::new_unchecked(core::ptr::dangling_mut()),
        )
    };
    let _ = old.block();
    // SAFETY[AUDIT:typed-layout-test-fixtures]: zero is a concrete valid flag
    // fixture used only to keep accessors referenced for lint accounting.
    let flags = unsafe { MallocGenericFlags::from_raw(0) };
    let _ = flags.raw();
    let _ = flags.huge_alignment();
}

#[cfg(test)]
mod tests;
