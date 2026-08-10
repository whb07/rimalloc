# Raw Facade Migration Inventory

This inventory tracks current `mechanical_mimalloc::raw` users and classifies
the raw modules after narrowing the default facade.

The default raw facade exposes root `raw::mi_*`, `raw::abi`, `raw::mimalloc`,
and `raw::headers`. Translated implementation modules are crate-private and no
longer exposed as raw Rust module paths.

## Old Path Migration Matrix

| Old path | New path | Classification |
|---|---|---|
| crate-root implementation modules such as `mechanical_mimalloc::alloc`, `mechanical_mimalloc::heap`, `mechanical_mimalloc::page`, `mechanical_mimalloc::internal` | No public replacement; use crate-root safe APIs or `mechanical_mimalloc::raw` as appropriate | Removed from public crate root; implementation internals are private. |
| `mechanical_mimalloc::raw::alloc::*` | root `mechanical_mimalloc::raw::mi_*` or `mechanical_mimalloc::raw::mimalloc::*` for public C allocation functions | Removed implementation-module path; use public raw C API. |
| `mechanical_mimalloc::raw::alloc_aligned::*` | root `mechanical_mimalloc::raw::mi_*_aligned*` or `mechanical_mimalloc::raw::mimalloc::*` | Removed implementation-module path; use public raw C API. |
| `mechanical_mimalloc::raw::free::*` | root `mechanical_mimalloc::raw::mi_free` and related public C free functions | Removed implementation-module path; use public raw C API. |
| `mechanical_mimalloc::raw::heap::*` | root `mechanical_mimalloc::raw::mi_heap_*` or `mechanical_mimalloc::raw::mimalloc::*` | Removed implementation-module path; use public raw C API. |
| `mechanical_mimalloc::raw::stats::*` | root `mechanical_mimalloc::raw::mi_stats_*`, `mechanical_mimalloc::raw::mi_heap_stats_*`, public stats constants/layouts, or `raw::abi` | Removed implementation-module path; use public raw C API/layouts. |
| `mechanical_mimalloc::raw::options::*` | root `mechanical_mimalloc::raw::mi_option_*`, `mechanical_mimalloc::raw::mi_option_t`, or `raw::abi` | Removed implementation-module path; use public raw C API/layouts. |
| `mechanical_mimalloc::raw::types::*` | root raw re-exports or `mechanical_mimalloc::raw::abi::*` for public C layouts and aliases | Removed implementation-module path; C-public layouts/types are exposed through raw root/`raw::abi`. |
| `mechanical_mimalloc::raw::headers::*` | `mechanical_mimalloc::raw::headers::*` | Raw public C/C++ header compatibility placeholders; retained in the default raw facade. |
| `mechanical_mimalloc::raw::abi::*` | `mechanical_mimalloc::raw::abi::*` | Raw public C declaration/layout owner; retained in the default raw facade. |
| `mechanical_mimalloc::raw::mimalloc::*` | root `mechanical_mimalloc::raw::mi_*` where convenient, or keep `raw::mimalloc::*` for grouped public C calls | Raw public C API alias; retained in the default raw facade. |
| `mechanical_mimalloc::raw::{arena, bitmap, bits, init, internal, libc, memory, os, page, page_map, prim, random, static_c, theap, threadlocal, track}` | No supported integration replacement | Removed implementation-module paths; internals are crate-private. |

## Current Raw Users

| Caller | Raw paths | Reason | Migration target |
|---|---|---|---|
| `README.md` | root `raw::mi_malloc`, `raw::mi_free` | Raw usage example | Keep as public C API example. |
| `src/lib.rs` crate docs | root `raw::mi_malloc`, `raw::mi_free` | Raw usage and compile-fail examples | Keep as public C API examples. |
| `src/ffi/abi.rs` macros | root `raw::mi_*`, `raw::mi_heap_*` | C header-style allocation helper macros | Keep only if macros remain public C-header compatibility; otherwise move through `raw::abi`/`raw::mimalloc`. |
| `benches/hotpath.rs` | root `raw::mi_*` allocation functions | Allocator hot-path benchmark shim | Keep on public C-header facade. |
| `benches/workloads.rs` | root `raw::mi_*` allocation functions | Allocator workload benchmark shim | Keep on public C-header facade. |
| `examples/aligned.rs` | root `raw::mi_malloc_aligned`, `raw::mi_realloc_aligned`, `raw::mi_free` | Public C API aligned allocation example | Keep as public C API example. |
| `examples/ffi.rs` | root `raw::mi_malloc`, `raw::mi_usable_size`, `raw::mi_free` | Public C API FFI example | Keep as public C API example. |
| `examples/raw_heap.rs` | root heap `raw::mi_heap_*`, `raw::mi_free` | Unsafe C heap API example | Keep as raw public C API example unless a safe heap API is stabilized. |
| `examples/raw_options.rs` | root option functions and `mi_option_t` | Unsafe C option API example | Keep as raw public C API example unless a safe options API is stabilized. |
| `examples/raw_stats.rs` | root stats functions, `mi_stats_t`, `MI_STAT_VERSION` | Unsafe C stats API example | Keep as raw public C API example unless a safe stats snapshot API is stabilized. |
| `tests/aligned_exports.rs` | `raw::mimalloc::*` | Public C API export behavior | Keep on public C-header facade. |
| `src/internal/bitmap.rs` unit tests | crate-private bitmap helpers | Internal bitmap regression coverage | Migrated from `tests/bitmap_regressions.rs`; no raw facade user. |
| `tests/compat_exports.rs` | `raw::mimalloc::*` | Public C API compatibility | Keep on public C-header facade. |
| `tests/ffi_panic_audit.rs` | repository source/docs only | Panic/unwind policy audit | Public source-policy integration test; `_mi_assert_fail` child-process probe migrated to `src/allocator/options.rs` unit tests. |
| `src/types/freelist/tests.rs` unit tests | crate-private free-list helpers | Internal free-list encoding coverage | Migrated from `tests/freelist_encoding.rs`; no raw facade user. |
| `tests/fuzz_alloc_sequences.rs` | `raw::mimalloc::*` | Public C API fuzz-style operation sequences | Keep on public C-header facade. |
| `tests/lifecycle_stats_exports.rs` | root `raw::*` public C-header facade | Public C stats/lifecycle export behavior | Migrated to public C-header facade. |
| `tests/no_allocation_critical_paths.rs` | `raw::mimalloc::*` | Public C API behavior under no-allocation constraints | Keep on public C-header facade. |
| `tests/options_exports.rs` | `raw::mimalloc::*` | Public C option export behavior | Keep on public C-header facade. |
| `tests/original_main.rs` | root `raw::mi_*` public C-header facade | Port of upstream C heap/allocation behavior | Migrated to public C-header facade. |
| `tests/original_main_override.rs` | root `raw::mi_*`, `raw::headers` | Port of upstream C override test plus C++ header placeholders | Public C calls migrated; header placeholders remain compatibility surface. |
| `tests/original_main_override_static.rs` | root `raw::mi_*` public C-header facade | Static override regression coverage | Public C calls stay on root raw paths; internal bin characterization migrated to `src/allocator/page.rs` unit tests. |
| `tests/original_main_static_dep.rs` | root `raw::mi_malloc`, `raw::mi_free` | Public allocation behavior | Migrated to public C-header facade. |
| `tests/original_test_api.rs` | root `raw::mi_*`, `raw::headers` | Port of upstream API test plus C++ header placeholders | Public C calls migrated; header placeholders remain compatibility surface. |
| `tests/original_test_api_fill.rs` | root `raw::mi_*` public C-header facade | Debug fill/tracking parity | Public C calls stay on root raw paths; debug/tracking byte constants are local test expectations. |
| `tests/original_test_stress.rs` | root `raw::mi_*` public C-header facade | Port of upstream stress test | Migrated to public C-header facade. |
| `tests/property_alloc_sequences.rs` | `raw::mimalloc::*` | Public C API property sequences | Keep on public C-header facade. |
| `src/types/constants.rs` unit tests | crate-private constants | Secure feature constants/types | Migrated from `tests/secure_features.rs`; no raw facade user. |
| `tests/stats_exports.rs` | root `raw::mi_stats_get_bin_size`, `raw::MI_BIN_HUGE` | Public stats export behavior | Migrated to public C-header facade. |
| `tests/thread_stats_regression.rs` | root `raw::mi_*`, `raw::mi_stats_t`, `raw::MI_STAT_VERSION` | Public stats regression behavior | Migrated to public C-header facade. |
| `src/types/track/tests.rs` unit tests | crate-private tracking constants | Tracking feature constants | Migrated from `tests/tracking_features.rs`; no raw facade user. |

## Raw Module Classification

| Raw module | Classification | Default-facade target |
|---|---|---|
| `raw::abi` | Public C header surface | Keep. |
| `raw::mimalloc` | Public C header surface alias | Keep. |
| `raw::headers` | C/C++ header macro compatibility placeholders | Keep until those placeholders are folded into `raw::abi` or removed. |
| `raw::types` | Removed implementation-module path | C-public layouts/types are re-exported at raw root through `raw::abi`. |
| `raw::alloc` | Removed implementation-module path | Public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::alloc_aligned` | Removed implementation-module path | Public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::alloc_posix` | Removed implementation-module path | Public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::alloc_override` | Removed implementation-module path | C symbols remain dynamic exports for artifact modes. |
| `raw::free` | Removed implementation-module path | Public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::heap` | Removed implementation-module path | Public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::stats` | Removed implementation-module path | C-public stats types/functions are re-exported at raw root through `raw::abi`. |
| `raw::options` | Removed implementation-module path | C-public option types/functions are re-exported at raw root through `raw::abi`. |
| `raw::track` | Removed implementation-module path | Crate-private tracking internals. |
| `raw::internal` | Removed implementation-module path | Crate-private helper internals. |
| `raw::page` | Removed implementation-module path | Crate-private allocator implementation. |
| `raw::page_map` | Removed implementation-module path | Crate-private allocator implementation. |
| `raw::theap` | Removed implementation-module path | Public C calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::arena` | Removed implementation-module path | Public C calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::arena_meta` | Removed implementation-module path | Crate-private arena metadata. |
| `raw::os` | Removed implementation-module path | Exported C symbols remain dynamic exports where applicable. |
| `raw::prim` | Removed implementation-module path | Crate-private platform primitive internals. |
| `raw::threadlocal` | Removed implementation-module path | Crate-private thread-local implementation. |
| `raw::random` | Removed implementation-module path | Crate-private random/hardening implementation. |
| `raw::memory` | Removed implementation-module path | Crate-private memory helpers. |
| `raw::bitmap` | Removed implementation-module path | Crate-private bitmap helpers. |
| `raw::bits` | Removed implementation-module path | Crate-private bit helpers. |
| `raw::libc` | Removed implementation-module path | Crate-private libc compatibility helpers. |
| `raw::init` | Removed implementation-module path | Public lifecycle calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::static_c` | Removed implementation-module path | Crate-private static facade compatibility. |

## Migration Order

1. Move remaining `raw::mimalloc::*` imports to root `raw::mi_*` imports where
   that improves readability.
2. Keep source-shape audits on direct source inspection or crate-private unit
   tests instead of raw implementation-module imports.
3. Do not reintroduce raw implementation-module paths without a new public API
   design and inventory update.
