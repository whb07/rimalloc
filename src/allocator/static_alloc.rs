//! Mechanical translation of `src/static.c`.

// Permanent compatibility shim: `src/static.c` was an aggregate C build unit.
// The Rust crate wires modules through `lib.rs`; these sentinels document that
// the static aggregate has no runtime implementation here.
#![allow(dead_code)]

pub const _DEFAULT_SOURCE: () = ();

pub const MI_STATIC_AGGREGATES_ALLOC: bool = true;
pub const MI_STATIC_AGGREGATES_ALLOC_ALIGNED: bool = true;
pub const MI_STATIC_AGGREGATES_ALLOC_POSIX: bool = true;
pub const MI_STATIC_AGGREGATES_ARENA: bool = true;
pub const MI_STATIC_AGGREGATES_ARENA_META: bool = true;
pub const MI_STATIC_AGGREGATES_BITMAP: bool = true;
pub const MI_STATIC_AGGREGATES_HEAP: bool = true;
pub const MI_STATIC_AGGREGATES_INIT: bool = true;
pub const MI_STATIC_AGGREGATES_LIBC: bool = true;
pub const MI_STATIC_AGGREGATES_OPTIONS: bool = true;
pub const MI_STATIC_AGGREGATES_OS: bool = true;
pub const MI_STATIC_AGGREGATES_PAGE: bool = true;
pub const MI_STATIC_AGGREGATES_PAGE_MAP: bool = true;
pub const MI_STATIC_AGGREGATES_RANDOM: bool = true;
pub const MI_STATIC_AGGREGATES_STATS: bool = true;
pub const MI_STATIC_AGGREGATES_THEAP: bool = true;
pub const MI_STATIC_AGGREGATES_THREADLOCAL: bool = true;
pub const MI_STATIC_AGGREGATES_PRIM: bool = true;
pub const MI_STATIC_AGGREGATES_OSX_ZONE: bool = false;
