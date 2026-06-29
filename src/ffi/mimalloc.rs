//! Internal compatibility alias for translated code.
//!
//! The C header mirror lives in `abi.rs`. This module exists only so the
//! mechanical translation can continue to use `crate::mimalloc::*` internally
//! while public users bind through `crate::raw` or `crate::raw::abi`.

pub(crate) use crate::abi::*;
