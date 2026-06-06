//! Mechanical translation of `src/static.c`.

#![allow(dead_code, non_upper_case_globals)]

// `src/static.c` is an aggregate build unit for the original C static override
// build. In the Rust crate the equivalent aggregation happens through `lib.rs`
// module wiring instead of textual source inclusion.

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
