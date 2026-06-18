//! Explicit C-shaped compatibility facade.
//!
//! Items in this module mirror the translated mimalloc C API, constants, raw
//! layouts, and low-level helpers used by parity tests and benchmarks. This is
//! intentionally separate from the crate root so implementation modules can
//! remain private without hiding the raw compatibility surface.
//!
//! The APIs here are not safe Rust abstractions. Most functions accept or
//! return raw pointers and inherit mimalloc's C ABI preconditions.

#![allow(unused_imports)]

pub mod abi {
    pub use crate::abi::*;
}

pub mod alloc {
    pub use crate::alloc::*;
}

pub mod alloc_aligned {
    pub use crate::alloc_aligned::*;
}

pub mod alloc_override {
    pub use crate::alloc_override::*;
}

pub mod alloc_posix {
    pub use crate::alloc_posix::*;
}

pub mod arena {
    pub use crate::arena::*;
}

pub mod arena_meta {
    pub use crate::arena_meta::*;
}

pub mod atomic {
    pub use crate::atomic::*;
}

pub mod bitmap {
    pub use crate::bitmap::*;
}

pub mod bits {
    pub use crate::bits::*;
}

pub mod free {
    pub use crate::free::*;
}

pub mod headers {
    pub use crate::headers::*;
}

pub mod heap {
    pub use crate::heap::*;
}

pub mod init {
    pub use crate::init::*;
}

pub mod internal {
    pub use crate::internal::*;
}

pub mod libc {
    pub use crate::libc::*;
}

pub mod memory {
    pub use crate::memory::*;
}

pub mod mimalloc {
    pub use crate::abi::*;
}

pub mod options {
    pub use crate::options::*;
}

pub mod os {
    pub use crate::os::*;
}

pub mod page {
    pub use crate::page::*;
}

pub mod page_map {
    pub use crate::page_map::*;
}

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

pub mod random {
    pub use crate::random::*;
}

pub mod static_c {
    pub use crate::static_c::*;
}

pub mod stats {
    pub use crate::stats::*;
}

pub mod theap {
    pub use crate::theap::*;
}

pub mod threadlocal {
    pub use crate::threadlocal::*;
}

pub mod track {
    pub use crate::track::*;
}

pub mod types {
    pub use crate::types::*;
}

pub use mimalloc::*;
