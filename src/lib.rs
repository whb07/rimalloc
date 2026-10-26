//! Mechanical Rust translation foundation for mimalloc 3.3.2.
//!
//! This crate provides safe Rust allocation, heap, options, statistics, and
//! process-info facades over a translated mimalloc allocator. It can also build
//! an explicit mimalloc-compatible C ABI/interposition artifact. Borrowed
//! callback guards live under [`unsafe_callbacks`] because they require
//! process-global safety obligations.
//!
//! The default library build produces:
//!
//! - an `rlib` with a deliberately small Rust-facing surface.
//!
//! Build the dynamic C ABI/interposition artifact explicitly:
//!
//! ```text
//! cargo rustc --features interpose --lib --release -- --crate-type cdylib
//! ```
//!
//! Use [`MiMalloc`] as a global allocator when a Rust `GlobalAlloc` facade is
//! needed, [`Allocation`] for owned byte allocations, [`Heap`] for safe heap
//! ownership, [`GlobalOptions`] and [`StatsSnapshot`] for non-printing
//! allocator controls and observations, or [`raw`] for the explicit C-shaped
//! compatibility surface. Implementation modules are crate-private so internal
//! translation structure does not become an accidental SemVer contract.
//!
//! # Features
//!
//! - `std`: opt in to `std` for tooling, tests, and benchmarks. Linux builds
//!   without this feature use `no_std`.
//! - `bench`: enables Criterion benchmarks and implies `std`.
//! - `compare`: enables comparison-oriented benchmark/test code.
//! - `bench-diagnostics`: enables benchmark-only hotspot counters.
//! - `secure`: enables secure-mode padding and encoded free lists.
//! - `guarded`: enables guarded allocation sampling.
//! - `track-valgrind`: enables Valgrind tracking shims.
//! - `track-asan`: enables ASan tracking shims.
//! - `c-header-macros`: exposes raw mimalloc C-header compatibility macros.
//! - `c-abi`: enables build-script setup for C ABI artifacts.
//! - `cxx-overrides`: compiles and links C++ operator new/delete overrides.
//! - `tls-shim`: compiles and links the Linux initial-exec TLS shim.
//! - `interpose`: enables the aggregate C ABI/interposition build mode.
//!
//! When `track-valgrind` and `track-asan` are both enabled by aggregate tooling
//! builds, Valgrind mode takes precedence.
//!
//! # Safety
//!
//! The exported C ABI and [`raw`] facade are unsafe by design: callers must
//! uphold mimalloc's pointer ownership, layout, alignment, threading, and
//! lifetime requirements. Safe Rust APIs encode ownership, initialization, and
//! heap lifetimes at the wrapper boundary.
//!
//! # Examples
//!
//! Use the Rust `GlobalAlloc` facade:
//!
//! ```rust,no_run
//! use rimalloc::MiMalloc;
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
//! ```rust,no_run
//! use core::alloc::Layout;
//! use rimalloc::Allocation;
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
//!     let p = rimalloc::raw::mi_malloc(64);
//!     rimalloc::raw::mi_free(p);
//! }
//! ```
//!
//! Raw calls are intentionally not safe Rust calls:
//!
//! ```rust,compile_fail
//! let p = rimalloc::raw::mi_malloc(64);
//! rimalloc::raw::mi_free(p);
//! ```
//!
//! With `c-header-macros` enabled, typed raw allocation macros also require
//! caller-side unsafe blocks:
//!
//! ```rust,ignore
//! unsafe {
//!     let p = rimalloc::mi_malloc_tp!(u64);
//!     rimalloc::raw::mi_free(p.cast());
//! }
//! ```
//!
//! ```rust,compile_fail
//! let p = rimalloc::mi_malloc_tp!(u64);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! Header override macros follow the same raw unsafe-boundary policy:
//!
//! ```rust,compile_fail
//! let p = rimalloc::malloc!(64);
//! rimalloc::_aligned_free!(p);
//! ```
//!
//! ```rust,compile_fail
//! let p = rimalloc::mi_zalloc_tp!(u64);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let p = rimalloc::mi_calloc_tp!(u64, 2);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let p = rimalloc::mi_mallocn_tp!(u64, 2);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let p = core::ptr::null_mut::<u64>();
//! let p = rimalloc::mi_reallocn_tp!(u64, p, 2);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let p = core::ptr::null_mut::<u64>();
//! let p = rimalloc::mi_reallocn_tp!(p, u64, 2);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let heap = core::ptr::null_mut();
//! let p = rimalloc::mi_heap_malloc_tp!(u64, heap);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let heap = core::ptr::null_mut();
//! let p = rimalloc::mi_heap_zalloc_tp!(u64, heap);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let heap = core::ptr::null_mut();
//! let p = rimalloc::mi_heap_calloc_tp!(u64, heap, 2);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let heap = core::ptr::null_mut();
//! let p = rimalloc::mi_heap_mallocn_tp!(u64, heap, 2);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let heap = core::ptr::null_mut();
//! let p = core::ptr::null_mut::<u64>();
//! let p = rimalloc::mi_heap_reallocn_tp!(u64, heap, p, 2);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! ```rust,compile_fail
//! let heap = core::ptr::null_mut();
//! let p = core::ptr::null_mut::<u64>();
//! let p = rimalloc::mi_heap_recalloc_tp!(u64, heap, p, 2);
//! rimalloc::raw::mi_free(p.cast());
//! ```
//!
//! Implementation modules are not part of the crate-root public API:
//!
//! ```rust,compile_fail
//! let _ = rimalloc::alloc::mi_malloc(64);
//! ```
//!
//! ```rust,compile_fail
//! use rimalloc::{heap, internal, options, os, page};
//! ```
//!
//! Internal typestate witnesses are also not public API. External users cannot
//! forge allocator-core states such as local pages, foreign pages, abandoned
//! pages, or validated free requests:
//!
//! ```rust,compile_fail
//! use rimalloc::types::{LocalPage, ForeignPage, OwnedAbandonedPage};
//! ```
//!
//! ```rust,compile_fail
//! use rimalloc::types::ValidatedFree;
//! ```

#![cfg_attr(mi_sanitizer_runtime, feature(sanitize))]
#![cfg_attr(all(not(test), target_os = "linux", not(feature = "std")), no_std)]
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unwrap_used
    )
)]
#![deny(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::bare_urls)]

pub mod raw;
pub use ffi::callbacks as unsafe_callbacks;

mod allocator;
mod ffi;
mod internal;
mod platform;
mod safe;
pub(crate) mod types;

/// Hidden helpers for Criterion benches only. Not part of the public API.
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod __bench {
    pub use crate::internal::_mi_memcpy;
}

// Raw and translated compatibility code retain these FFI module names.
pub(crate) use ffi::{abi, headers, mimalloc};

// Translated atomic/bitmap/bit/random helpers retain their upstream module
// vocabulary. New Rust-native helpers should use owning `internal` paths.
pub(crate) use internal::{atomic, bitmap, bits, random};

// Platform compatibility code mirrors upstream `prim` and `os` names.
pub(crate) use platform as prim;
pub(crate) use platform::os;

// Internal interposition code retains the translated override-macro path.
#[allow(unused_imports)] // Used only by target-specific override expansions.
pub(crate) use raw::macros as alloc_override;

// Mechanical code uses `core_types` as the typed invariant namespace; new
// Rust-native code should depend on the owning `types` module.
pub(crate) use types as core_types;

pub use safe::{
    AllocError, Allocation, GlobalOptions, Heap, HeapAllocation, HeapError, HeapRef,
    InitializedAllocation, InitializedHeapAllocation, LayoutError, MainHeap, OptionError,
    OptionKey, ProcessInfo, ResizeError, ResizeErrorKind, StatCount, StatTotal, StatsError,
    StatsSnapshot, TryAllocError,
};

/// Rust [`GlobalAlloc`](core::alloc::GlobalAlloc) facade backed by the crate's
/// mimalloc implementation.
///
/// This wrapper is intentionally small and process-wide. Use [`Heap`] for
/// heap-scoped allocation ownership, [`GlobalOptions`] for supported
/// process-global safe options, and [`StatsSnapshot`] for non-printing
/// statistics.
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
/// The facade itself does not intentionally panic. `no_std` Linux builds use
/// the crate panic handler; `std` tooling, tests, docs, and examples use normal
/// Rust panic behavior outside allocator calls.
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
/// use rimalloc::MiMalloc;
///
/// #[global_allocator]
/// static GLOBAL: MiMalloc = MiMalloc;
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct MiMalloc;

unsafe impl core::alloc::GlobalAlloc for MiMalloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe {
            if allocator::aligned::mi_malloc_satisfies_alignment(layout.size(), layout.align()) {
                allocator::alloc::mi_malloc(layout.size()).cast()
            } else {
                allocator::aligned::mi_malloc_aligned(layout.size(), layout.align()).cast()
            }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe {
            if allocator::aligned::mi_malloc_satisfies_alignment(layout.size(), layout.align()) {
                allocator::alloc::mi_zalloc(layout.size()).cast()
            } else {
                allocator::aligned::mi_zalloc_aligned(layout.size(), layout.align()).cast()
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        unsafe { allocator::free::mi_free(ptr.cast()) }
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        unsafe {
            if allocator::aligned::mi_malloc_satisfies_alignment(new_size, layout.align()) {
                allocator::alloc::mi_realloc(ptr.cast(), new_size).cast()
            } else {
                allocator::aligned::mi_realloc_aligned(ptr.cast(), new_size, layout.align()).cast()
            }
        }
    }
}

#[cfg(all(not(test), target_os = "linux", not(feature = "std")))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    unsafe {
        allocator::options::_mi_assert_fail(
            c"panic".as_ptr(),
            c"rust".as_ptr(),
            0,
            core::ptr::null(),
        )
    }
}

#[cfg(all(not(test), target_os = "linux", not(feature = "std")))]
unsafe extern "C" {
    #[link_name = "abort"]
    fn c_abort() -> !;
}

#[cfg(all(not(test), target_os = "linux", not(feature = "std")))]
#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() {
    unsafe { c_abort() }
}
