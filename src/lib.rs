//! Mechanical Rust translation foundation for mimalloc 3.3.2

#![cfg_attr(
    all(
        not(test),
        not(feature = "std"),
        target_os = "linux",
        not(debug_assertions)
    ),
    no_std
)]
#![allow(clippy::nonminimal_bool)]

pub mod abi;
pub mod alloc;
pub mod alloc_aligned;
pub mod alloc_override;
pub mod alloc_posix;
pub mod arena;
pub mod arena_meta;
pub mod atomic;
pub mod bitmap;
pub mod bits;
pub(crate) mod core_types;
pub mod free;
pub mod headers;
pub mod heap;
pub mod init;
pub mod internal;
pub mod libc;
pub mod memory;
pub mod mimalloc;
pub mod options;
pub mod os;
pub mod page;
pub mod page_map;
pub mod prim;
pub mod random;
#[path = "static.rs"]
pub mod static_c;
pub mod stats;
pub mod theap;
pub mod threadlocal;
pub mod track;
pub mod types;

#[cfg(all(
    not(test),
    not(feature = "std"),
    target_os = "linux",
    not(debug_assertions)
))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    unsafe { options::_mi_assert_fail(c"panic".as_ptr(), c"rust".as_ptr(), 0, core::ptr::null()) }
}

#[cfg(all(
    not(test),
    not(feature = "std"),
    target_os = "linux",
    not(debug_assertions)
))]
unsafe extern "C" {
    #[link_name = "abort"]
    fn c_abort() -> !;
}

#[cfg(all(
    not(test),
    not(feature = "std"),
    target_os = "linux",
    not(debug_assertions)
))]
#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() {
    unsafe { c_abort() }
}
