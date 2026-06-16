//! DT-1 scalar domain types (zero-cost newtypes).
//! This is the first incremental step of the TYPES_SPEC.md plan.

mod scalar;

pub(crate) use scalar::{
    Alignment, ArenaIndex, Bin, BlockSize, DirectBin, EffectiveSize, MaybeAlignment,
    PaddedSize, RequestedSize, SliceCount, SliceIndex, UsableSize,
};