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
//! The default facade exposes the public C-header mirror through [`abi`],
//! [`mimalloc`], and root `mi_*` re-exports. Translated implementation modules
//! are available only with the `unstable-raw-internals` feature for repository
//! audits and are not public API. C ABI symbol compatibility is tracked
//! separately.
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
//!     let p = mechanical_mimalloc::raw::mi_malloc(64);
//!     mechanical_mimalloc::raw::mi_free(p);
//! }
//! ```
//!
//! Like C `free`, `mi_free` accepts null:
//!
//! ```rust,no_run
//! unsafe {
//!     mechanical_mimalloc::raw::mi_free(core::ptr::null_mut());
//! }
//! ```
//!
//! Like C `realloc`, `mi_realloc` accepts null as an allocation request:
//!
//! ```rust,no_run
//! unsafe {
//!     let p = mechanical_mimalloc::raw::mi_realloc(core::ptr::null_mut(), 64);
//!     mechanical_mimalloc::raw::mi_free(p);
//! }
//! ```

#![allow(unused_imports)]

pub mod abi {
    pub use crate::abi::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod alloc {
    pub use crate::alloc::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod alloc_aligned {
    pub use crate::alloc_aligned::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod alloc_override {
    pub use crate::alloc_override::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod alloc_posix {
    pub use crate::alloc_posix::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod arena {
    pub use crate::arena::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod arena_meta {
    pub use crate::arena_meta::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod atomic {
    pub use crate::atomic::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod bitmap {
    pub use crate::bitmap::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod bits {
    pub use crate::bits::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod free {
    pub use crate::free::*;
}

pub mod headers {
    pub use crate::headers::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod heap {
    pub use crate::heap::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod init {
    pub use crate::init::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod internal {
    pub use crate::internal::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod libc {
    pub use crate::libc::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod memory {
    pub use crate::memory::*;
}

pub mod mimalloc {
    pub use crate::abi::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod options {
    pub use crate::options::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod os {
    pub use crate::os::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod page {
    pub use crate::page::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod page_map {
    pub use crate::page_map::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod prim {
    #[cfg(target_os = "emscripten")]
    pub mod emscripten {
        pub use crate::prim::emscripten::*;
    }

    #[cfg(target_os = "macos")]
    pub mod osx {
        pub use crate::prim::osx::*;
    }

    #[cfg(all(not(windows), not(target_os = "emscripten"), not(target_os = "wasi")))]
    pub mod unix {
        pub use crate::prim::unix::*;
    }

    #[cfg(target_os = "wasi")]
    pub mod wasi {
        pub use crate::prim::wasi::*;
    }

    #[cfg(windows)]
    pub mod windows {
        pub use crate::prim::windows::*;
    }

    pub use crate::prim::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod random {
    pub use crate::random::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod static_c {
    pub use crate::static_c::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod stats {
    pub use crate::stats::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod theap {
    pub use crate::theap::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod threadlocal {
    pub use crate::threadlocal::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod track {
    pub use crate::track::*;
}

#[cfg(feature = "unstable-raw-internals")]
#[deprecated(
    note = "raw implementation module path; not public API; use root raw::mi_* or raw::abi for C-header surface"
)]
pub mod types {
    pub use crate::types::*;
}

pub use mimalloc::*;
