//! DT-14: Padding types.

use core::ptr::NonNull;

use crate::core_types::{PageKeys, UsableSize};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PaddingCanary(pub(crate) u32);

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct PaddingDelta(pub(crate) u32);

#[repr(transparent)]
pub(crate) struct PaddingPtr(pub(crate) NonNull<crate::types::mi_padding_t>);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Padding {
    pub(crate) canary: PaddingCanary,
    pub(crate) delta: PaddingDelta,
}
