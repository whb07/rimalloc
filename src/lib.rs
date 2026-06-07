//! Mechanical Rust translation foundation for mimalloc 3.3.2

#![allow(clippy::nonminimal_bool)]

pub mod abi;
pub mod alloc;
pub mod alloc_aligned;
pub mod alloc_override;
pub mod alloc_posix;
pub mod arena;
pub mod atomic;
pub mod bitmap;
pub mod bits;
pub mod free;
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
