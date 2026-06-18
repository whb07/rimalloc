//! Mechanical Rust translation foundation for mimalloc 3.3.2.
//!
//! This crate is currently a dual artifact:
//!
//! - a `cdylib` that exports mimalloc-compatible C ABI symbols, and
//! - an `rlib` with a deliberately small Rust-facing surface for tests,
//!   benchmarks, and experimental integration.
//!
//! The Rust API is not yet stabilized. Use [`MiMalloc`] as a global allocator
//! when a Rust `GlobalAlloc` facade is needed, or use [`raw`] for the explicit
//! C-shaped compatibility surface. Implementation modules are crate-private so
//! internal translation structure does not become an accidental SemVer contract.
//!
//! # Features
//!
//! - `std`: opt in to `std` for tooling, tests, and benchmarks. Optimized Linux
//!   builds without this feature use `no_std`.
//! - `bench`: enables Criterion benchmarks and implies `std`.
//! - `compare`: enables comparison-oriented benchmark/test code.
//! - `secure`: enables secure-mode padding and encoded free lists.
//! - `guarded`: enables guarded allocation sampling.
//! - `track-valgrind`: enables Valgrind tracking shims.
//! - `track-asan`: enables ASan tracking shims.
//!
//! `track-valgrind` and `track-asan` are mutually exclusive.
//!
//! # Safety
//!
//! The exported C ABI and [`raw`] facade are unsafe by design: callers must
//! uphold mimalloc's pointer ownership, layout, alignment, threading, and
//! lifetime requirements. Safe Rust APIs are intentionally limited until those
//! invariants are documented and tested at the wrapper boundary.

#![cfg_attr(
    all(
        not(test),
        target_os = "linux",
        not(debug_assertions),
        not(feature = "std")
    ),
    no_std
)]
#![deny(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::bare_urls)]

pub mod raw;

mod abi;
mod alloc;
mod alloc_aligned;
mod alloc_override;
mod alloc_posix;
mod arena;
mod arena_meta;
mod atomic;
mod bitmap;
mod bits;
pub(crate) mod core_types;
mod free;
mod headers;
mod heap;
mod init;
mod internal;
mod libc;
mod memory;
mod mimalloc;
mod options;
mod os;
mod page;
mod page_map;
mod prim;
mod random;
#[path = "static.rs"]
mod static_c;
mod stats;
mod theap;
mod threadlocal;
mod track;
mod types;

/// Rust `GlobalAlloc` facade backed by the crate's mimalloc implementation.
///
/// This wrapper is intentionally small. It does not expose heap handles,
/// options, or statistics as safe Rust APIs yet because those invariants are
/// still being audited. Use [`raw`] for the C-shaped surface while that work
/// continues.
pub struct MiMalloc;

unsafe impl core::alloc::GlobalAlloc for MiMalloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe {
            if layout.align() <= types::MI_MAX_ALIGN_SIZE {
                alloc::mi_malloc(layout.size()).cast()
            } else {
                alloc_aligned::mi_malloc_aligned(layout.size(), layout.align()).cast()
            }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe {
            if layout.align() <= types::MI_MAX_ALIGN_SIZE {
                alloc::mi_zalloc(layout.size()).cast()
            } else {
                alloc_aligned::mi_zalloc_aligned(layout.size(), layout.align()).cast()
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        unsafe { free::mi_free(ptr.cast()) }
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        unsafe {
            if layout.align() <= types::MI_MAX_ALIGN_SIZE {
                alloc::mi_realloc(ptr.cast(), new_size).cast()
            } else {
                alloc_aligned::mi_realloc_aligned(ptr.cast(), new_size, layout.align()).cast()
            }
        }
    }
}

#[cfg(all(
    not(test),
    target_os = "linux",
    not(debug_assertions),
    not(feature = "std")
))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    unsafe { options::_mi_assert_fail(c"panic".as_ptr(), c"rust".as_ptr(), 0, core::ptr::null()) }
}

#[cfg(all(
    not(test),
    target_os = "linux",
    not(debug_assertions),
    not(feature = "std")
))]
unsafe extern "C" {
    #[link_name = "abort"]
    fn c_abort() -> !;
}

#[cfg(all(
    not(test),
    target_os = "linux",
    not(debug_assertions),
    not(feature = "std")
))]
#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() {
    unsafe { c_abort() }
}
