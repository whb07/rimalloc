//! DT-3 allocation layout types.

use crate::core_types::{
    BlockPtr, EffectiveSize, MaybeAlignment, PaddedSize, PagePtr, RequestedSize, UsableSize,
    UserPtr,
};

/// Represents an explicit zeroing request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Zeroed {
    No,
    Yes,
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
        crate::mi_debug_assert!(
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

/// Errors when creating layout structures.
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
    pub(crate) fn malloc(size: usize) -> Result<Self, LayoutError> {
        let requested = RequestedSize::new(size);
        // Effective size for malloc is at least 1, up to the implementation limits.
        let eff_size = if size == 0 { 1 } else { size };
        // Unsafe is fine here as we've explicitly guarded against 0.
        let effective = unsafe { EffectiveSize::new_unchecked(eff_size) };
        let padded = requested
            .checked_padded()
            .map_err(|_| LayoutError::SizeOverflow)?;

        Ok(Self {
            requested,
            effective,
            padded,
            alignment: MaybeAlignment::new(0),
            zeroed: Zeroed::No,
        })
    }

    pub(crate) fn zalloc(size: usize) -> Result<Self, LayoutError> {
        let mut req = Self::malloc(size)?;
        req.zeroed = Zeroed::Yes;
        Ok(req)
    }

    pub(crate) fn aligned(size: usize, alignment: usize) -> Result<Self, LayoutError> {
        if alignment != 0 && !alignment.is_power_of_two() {
            return Err(LayoutError::AlignmentInvalid);
        }
        let mut req = Self::malloc(size)?;
        req.alignment = MaybeAlignment::new(alignment);
        Ok(req)
    }

    pub(crate) fn aligned_at(
        size: usize,
        alignment: usize,
        offset: usize,
        zeroed: Zeroed,
    ) -> Result<Self, LayoutError> {
        if alignment != 0 && !alignment.is_power_of_two() {
            return Err(LayoutError::AlignmentInvalid);
        }
        // Basic overflow check for the `aligned_at` calculation
        if size.checked_add(offset).is_none() || size.checked_add(alignment).is_none() {
            return Err(LayoutError::AlignmentOverflow);
        }

        let mut req = Self::malloc(size)?;
        req.alignment = MaybeAlignment::new(alignment);
        req.zeroed = zeroed;
        Ok(req)
    }

    /// Construct a layout from a size that already includes mimalloc padding.
    ///
    /// # Safety
    /// `padded` must be the result of adding `MI_PADDING_SIZE` to the logical
    /// requested size for this allocation path.
    pub(crate) unsafe fn from_padded_unchecked(padded: PaddedSize, zeroed: Zeroed) -> Self {
        crate::mi_debug_assert!(
            padded.get() >= crate::types::MI_PADDING_SIZE,
            "padded allocation size must include padding"
        );
        let requested_size = padded.get().wrapping_sub(crate::types::MI_PADDING_SIZE);
        let effective_size = if requested_size == 0 {
            1
        } else {
            requested_size
        };
        Self {
            requested: RequestedSize::new(requested_size),
            effective: EffectiveSize::new_unchecked(effective_size),
            padded,
            alignment: MaybeAlignment::new(0),
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
pub(crate) struct OldAllocation {
    ptr: UserPtr,
    usable: UsableSize,
    page: PagePtr,
    block: BlockPtr,
}

impl OldAllocation {
    /// Constructs OldAllocation. Only safe if the caller validates `ptr` maps to `page`/`block`.
    pub(crate) unsafe fn new(
        ptr: UserPtr,
        usable: UsableSize,
        page: PagePtr,
        block: BlockPtr,
    ) -> Self {
        Self {
            ptr,
            usable,
            page,
            block,
        }
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

pub(crate) struct ReallocRequest {
    ptr: Option<UserPtr>,
    old: Option<OldAllocation>,
    new_layout: LayoutRequest,
    zero_new_tail: Zeroed,
}

impl ReallocRequest {
    pub(crate) fn new(
        ptr: Option<UserPtr>,
        old: Option<OldAllocation>,
        new_layout: LayoutRequest,
        zero_new_tail: Zeroed,
    ) -> Self {
        Self {
            ptr,
            old,
            new_layout,
            zero_new_tail,
        }
    }

    pub(crate) fn ptr(&self) -> Option<UserPtr> {
        self.ptr
    }

    pub(crate) fn old(&self) -> Option<OldAllocation> {
        self.old
    }

    pub(crate) fn new_layout(&self) -> LayoutRequest {
        self.new_layout
    }

    pub(crate) fn zero_new_tail(&self) -> Zeroed {
        self.zero_new_tail
    }

    pub(crate) fn with_old(mut self, old: Option<OldAllocation>) -> Self {
        self.old = old;
        self
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
        crate::mi_debug_assert!(
            (huge_alignment.get() & Self::ZERO_BIT) == 0,
            "huge alignment must not collide with the zeroing flag"
        );
        Self(z | huge_alignment.get())
    }

    #[inline(always)]
    pub(crate) fn from_raw(raw: usize) -> Self {
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
