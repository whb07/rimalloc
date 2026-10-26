//! Explicit C-shaped compatibility facade.
//!
//! Items in this module mirror the translated mimalloc C API, constants, raw
//! layouts, and low-level helpers used by parity tests and benchmarks. This is
//! intentionally separate from the crate root so implementation modules can
//! remain private without hiding the raw compatibility surface.
//!
//! The APIs here are not safe Rust abstractions. Most functions accept or
//! return raw pointers and inherit mimalloc's C ABI preconditions.
//! Public raw signatures intentionally use `*mut` and `*const` rather than
//! `NonNull` so they can represent C nullability exactly.
//! The facade exposes the public C-header mirror through [`abi`], [`mimalloc`],
//! [`headers`], and root `mi_*` re-exports. Translated implementation modules
//! stay crate-private and are not public API. C ABI symbol compatibility is
//! tracked separately.
//!
//! Null is valid only where the C API says it is valid. Allocation functions
//! return null on failure, `mi_free` accepts null as a no-op, and
//! `mi_realloc` with a null input pointer behaves like allocation. Metadata
//! pointers, out-pointers, and callback context pointers are valid, non-null
//! caller obligations unless the specific C-shaped API documents or inherits a
//! nullable case.
//! Raw callbacks registered through this facade must not panic or unwind across
//! the C ABI boundary.
//!
//! Correct raw allocation pairs must remain inside an explicit unsafe boundary:
//!
//! ```rust,no_run
//! unsafe {
//!     let p = rimalloc::raw::mi_malloc(64);
//!     rimalloc::raw::mi_free(p);
//! }
//! ```
//!
//! Like C `free`, `mi_free` accepts null:
//!
//! ```rust,no_run
//! unsafe {
//!     rimalloc::raw::mi_free(core::ptr::null_mut());
//! }
//! ```
//!
//! Like C `realloc`, `mi_realloc` accepts null as an allocation request:
//!
//! ```rust,no_run
//! unsafe {
//!     let p = rimalloc::raw::mi_realloc(core::ptr::null_mut(), 64);
//!     rimalloc::raw::mi_free(p);
//! }
//! ```

#![allow(unused_imports)]
#![deny(clippy::must_use_candidate)]

pub(crate) mod macros;
pub(crate) mod posix;

pub mod abi {
    pub use crate::abi::*;
}

pub mod headers {
    pub use crate::headers::*;
}

pub mod mimalloc {
    pub use crate::abi::*;
}

pub use mimalloc::*;

// Benchmark-only opaque fixture exports. These are excluded from normal builds
// and remain hidden alongside the established raw facade re-exports.
#[cfg(feature = "bench-diagnostics")]
#[doc(hidden)]
pub use crate::allocator::bench_diagnostics::{
    mi_bench_page_batch_fixture_destroy, mi_bench_page_batch_fixture_new,
    mi_bench_page_batch_fixture_prepare, mi_bench_page_batch_fixture_run,
};
