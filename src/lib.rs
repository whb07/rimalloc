//! Mechanical Rust translation foundation for mimalloc 3.3.2.
//!
//! This crate is a mimalloc-compatible C ABI allocator artifact. It also
//! provides a narrow, experimental safe Rust allocation facade. It is not yet a
//! broad safe Rust heap, options, or stats abstraction.
//!
//! This crate is currently a dual artifact:
//!
//! - a `cdylib` that exports mimalloc-compatible C ABI symbols, and
//! - an `rlib` with a deliberately small Rust-facing surface for tests,
//!   benchmarks, and experimental integration.
//!
//! The Rust API is not yet stabilized. Use [`MiMalloc`] as a global allocator
//! when a Rust `GlobalAlloc` facade is needed, [`Allocation`] when a small
//! owned byte allocation wrapper is enough, or [`raw`] for the explicit C-shaped
//! compatibility surface. Implementation modules are crate-private so internal
//! translation structure does not become an accidental SemVer contract.
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
//!
//! # Examples
//!
//! Use the Rust `GlobalAlloc` facade:
//!
//! ```rust,no_run
//! use mechanical_mimalloc::MiMalloc;
//!
//! #[global_allocator]
//! static GLOBAL: MiMalloc = MiMalloc;
//!
//! fn main() {
//!     let values = Box::new([1usize, 2, 3, 4]);
//!     assert_eq!(values.iter().sum::<usize>(), 10);
//! }
//! ```
//!
//! Use the safe owned allocation wrapper when you want a Rust `Result` instead
//! of the C API's null-on-failure convention:
//!
//! ```rust,ignore
//! use core::alloc::Layout;
//! use mechanical_mimalloc::Allocation;
//!
//! let layout = Layout::from_size_align(128, 16).unwrap();
//! let allocation = Allocation::zeroed(layout).unwrap();
//! assert_eq!(allocation.len(), 128);
//! ```
//!
//! Use the raw C-shaped facade only behind an explicit unsafe boundary:
//!
//! ```rust,no_run
//! unsafe {
//!     let p = mechanical_mimalloc::raw::mi_malloc(64);
//!     mechanical_mimalloc::raw::mi_free(p);
//! }
//! ```
//!
//! Raw calls are intentionally not safe Rust calls:
//!
//! ```rust,compile_fail
//! let p = mechanical_mimalloc::raw::mi_malloc(64);
//! mechanical_mimalloc::raw::mi_free(p);
//! ```
//!
//! Implementation modules are not part of the crate-root public API:
//!
//! ```rust,compile_fail
//! let _ = mechanical_mimalloc::alloc::mi_malloc(64);
//! ```
//!
//! ```rust,compile_fail
//! use mechanical_mimalloc::{heap, internal, options, os, page};
//! ```
//!
//! Internal typestate witnesses are also not public API. External users cannot
//! forge allocator-core states such as local pages, foreign pages, abandoned
//! pages, or validated free requests:
//!
//! ```rust,compile_fail
//! use mechanical_mimalloc::core_types::{LocalPage, ForeignPage, OwnedAbandonedPage};
//! ```
//!
//! ```rust,compile_fail
//! use mechanical_mimalloc::core_types::ValidatedFree;
//! ```

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
mod safe;
#[path = "static.rs"]
mod static_c;
mod stats;
mod theap;
mod threadlocal;
mod track;
mod types;

pub use safe::{AllocError, Allocation, LayoutError, TryAllocError};

/// Rust [`GlobalAlloc`](core::alloc::GlobalAlloc) facade backed by the crate's
/// mimalloc implementation.
///
/// This wrapper is intentionally small. It does not expose heap handles,
/// options, or statistics as safe Rust APIs yet because those invariants are
/// still being audited. Use [`raw`] for the C-shaped surface while that work
/// continues.
///
/// # `GlobalAlloc` scope
///
/// `MiMalloc` implements Rust's process-wide global allocator interface. When
/// declared with `#[global_allocator]`, Rust allocation APIs such as `Box`,
/// `Vec`, and `String` allocate through this facade after the static allocator
/// has been selected.
///
/// # Zero-size layouts
///
/// The [`GlobalAlloc`](core::alloc::GlobalAlloc) contract requires callers to pass non-zero-size layouts
/// to `alloc` and `alloc_zeroed`; zero-size allocation behavior is therefore
/// inherited from the unsafe trait contract. Safe wrappers in this crate handle
/// zero-size layouts separately.
///
/// # Alignment
///
/// Layouts with alignment up to mimalloc's default maximum alignment use the
/// normal allocation path. Over-aligned layouts use mimalloc's aligned
/// allocation and reallocation paths.
///
/// # Allocation failure
///
/// Allocation and reallocation return a null pointer on failure, matching the
/// [`GlobalAlloc`](core::alloc::GlobalAlloc) contract. The facade does not panic on allocation failure.
///
/// # Deallocation layouts
///
/// `dealloc` expects the pointer and layout to describe a live allocation that
/// came from this allocator, as required by [`GlobalAlloc`](core::alloc::GlobalAlloc). Passing any other
/// pointer, an already-freed pointer, or a mismatched layout is caller UB under
/// Rust's allocator contract.
///
/// # Reallocation
///
/// `realloc` preserves the old contents up to `min(old_size, new_size)` on
/// success, following mimalloc's reallocation behavior. If reallocation fails,
/// it returns null and leaves the original allocation valid for the caller to
/// free.
///
/// # Panic and unwind behavior
///
/// The facade itself does not intentionally panic. Optimized `no_std` Linux
/// builds use the crate panic handler; `std` tooling, tests, docs, and examples
/// use normal Rust panic behavior outside allocator calls.
///
/// # Thread behavior
///
/// `MiMalloc` is a zero-sized facade over mimalloc's process allocator and is
/// safe to use as a global allocator from multiple threads. Thread-local
/// allocator state remains internal to mimalloc.
///
/// # Feature interactions
///
/// The `secure`, `guarded`, `track-valgrind`, and `track-asan` features may
/// change allocator hardening or diagnostics, but they do not change the
/// `GlobalAlloc` ownership, alignment, failure, or deallocation contracts.
/// The `std` feature affects documentation, examples, tests, and error trait
/// support, not the `GlobalAlloc` contract.
///
/// ```rust,no_run
/// use mechanical_mimalloc::MiMalloc;
///
/// #[global_allocator]
/// static GLOBAL: MiMalloc = MiMalloc;
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct MiMalloc;

// SAFETY[AUDIT:global-alloc]: `MiMalloc` is a zero-sized facade over the
// process allocator. Each unsafe trait method delegates to mimalloc while
// preserving Rust's `GlobalAlloc` null-on-failure and caller-layout contract.
unsafe impl core::alloc::GlobalAlloc for MiMalloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        // SAFETY[AUDIT:global-alloc]: `GlobalAlloc` callers provide a valid
        // non-zero `Layout`; over-aligned requests use mimalloc's aligned path.
        unsafe {
            if layout.align() <= types::MI_MAX_ALIGN_SIZE {
                alloc::mi_malloc(layout.size()).cast()
            } else {
                alloc_aligned::mi_malloc_aligned(layout.size(), layout.align()).cast()
            }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: core::alloc::Layout) -> *mut u8 {
        // SAFETY[AUDIT:global-alloc]: same layout contract as `alloc`, with
        // the zeroing path selected before returning to the caller.
        unsafe {
            if layout.align() <= types::MI_MAX_ALIGN_SIZE {
                alloc::mi_zalloc(layout.size()).cast()
            } else {
                alloc_aligned::mi_zalloc_aligned(layout.size(), layout.align()).cast()
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        // SAFETY[AUDIT:global-alloc]: `GlobalAlloc` requires `ptr` to be a
        // live allocation from this allocator with its original layout.
        unsafe { free::mi_free(ptr.cast()) }
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        // SAFETY[AUDIT:global-alloc]: `GlobalAlloc` requires `ptr` and
        // `layout` to identify a live allocation; mimalloc preserves contents
        // on success and returns null on failure.
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
    // SAFETY[AUDIT:panic-boundary]: the no_std panic handler forwards a static
    // message to mimalloc's aborting assertion path and never unwinds.
    unsafe { options::_mi_assert_fail(c"panic".as_ptr(), c"rust".as_ptr(), 0, core::ptr::null()) }
}

#[cfg(all(
    not(test),
    target_os = "linux",
    not(debug_assertions),
    not(feature = "std")
))]
// SAFETY[AUDIT:panic-boundary]: declaration of the platform abort symbol used
// only by the no_std personality shim.
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
    // SAFETY[AUDIT:panic-boundary]: optimized no_std builds abort if an
    // unwinding personality is reached.
    unsafe { c_abort() }
}
