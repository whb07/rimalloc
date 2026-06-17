//! DT-3 allocation layout types.

use crate::core_types::{
    BlockPtr, EffectiveSize, MaybeAlignment, PaddedSize, PagePtr, RequestedSize, UsableSize,
    UserPtr,
};
use crate::types::MI_PADDING_SIZE;

/// Represents an explicit zeroing request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Zeroed {
    No,
    Yes,
}

/// Represents an alignment request that might be huge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MaybeHugeAlignment {
    Normal,
    Huge,
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
}

/// Generic malloc flags enforcing mutual exclusion of alignment bits and zeroing bit.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct MallocGenericFlags(usize);

impl MallocGenericFlags {
    pub(crate) fn new(zeroed: Zeroed, huge_alignment: MaybeHugeAlignment) -> Self {
        let z = if zeroed == Zeroed::Yes { 1 } else { 0 };
        let a = if huge_alignment == MaybeHugeAlignment::Huge {
            2
        } else {
            0
        };
        Self(z | a)
    }

    pub(crate) fn zeroed(self) -> Zeroed {
        if (self.0 & 1) != 0 {
            Zeroed::Yes
        } else {
            Zeroed::No
        }
    }

    pub(crate) fn huge_alignment(self) -> MaybeHugeAlignment {
        if (self.0 & 2) != 0 {
            MaybeHugeAlignment::Huge
        } else {
            MaybeHugeAlignment::Normal
        }
    }
}
impl LayoutRequest {
    pub(crate) fn requested(self) -> RequestedSize {
        self.requested
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
}
