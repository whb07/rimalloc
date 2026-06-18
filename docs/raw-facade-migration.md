# Raw Facade Migration Inventory

This inventory tracks current `mechanical_mimalloc::raw` users and classifies
the raw modules after narrowing the default facade.

The default raw facade exposes root `raw::mi_*`, `raw::abi`, `raw::mimalloc`,
and `raw::headers`. The `unstable-raw-internals` feature exposes translated
implementation modules for repository audits and parity tests only. It is off
by default and carries no SemVer guarantees.

## Current Raw Users

| Caller | Raw paths | Reason | Migration target |
|---|---|---|---|
| `README.md` | root `raw::mi_malloc`, `raw::mi_free` | Raw usage example | Keep as public C API example. |
| `src/lib.rs` crate docs | root `raw::mi_malloc`, `raw::mi_free` | Raw usage and compile-fail examples | Keep as public C API examples. |
| `src/abi.rs` macros | root `raw::mi_*`, `raw::mi_heap_*` | C header-style allocation helper macros | Keep only if macros remain public C-header compatibility; otherwise move through `raw::abi`/`raw::mimalloc`. |
| `benches/hotpath.rs` | root `raw::mi_*` allocation functions | Allocator hot-path benchmark shim | Keep on public C-header facade. |
| `benches/workloads.rs` | root `raw::mi_*` allocation functions | Allocator workload benchmark shim | Keep on public C-header facade. |
| `examples/aligned.rs` | root `raw::mi_malloc_aligned`, `raw::mi_realloc_aligned`, `raw::mi_free` | Public C API aligned allocation example | Keep as public C API example. |
| `examples/ffi.rs` | root `raw::mi_malloc`, `raw::mi_usable_size`, `raw::mi_free` | Public C API FFI example | Keep as public C API example. |
| `examples/raw_heap.rs` | root heap `raw::mi_heap_*`, `raw::mi_free` | Unsafe C heap API example | Keep as raw public C API example unless a safe heap API is stabilized. |
| `examples/raw_options.rs` | root option functions and `mi_option_t` | Unsafe C option API example | Keep as raw public C API example unless a safe options API is stabilized. |
| `examples/raw_stats.rs` | root stats functions, `mi_stats_t`, `MI_STAT_VERSION` | Unsafe C stats API example | Keep as raw public C API example unless a safe stats snapshot API is stabilized. |
| `tests/aligned_exports.rs` | `raw::mimalloc::*` | Public C API export behavior | Keep on public C-header facade. |
| `tests/bitmap_regressions.rs` | `raw::bitmap` under `unstable-raw-internals` | Internal bitmap regression coverage | Gated behind unstable raw internals. |
| `tests/compat_exports.rs` | `raw::mimalloc::*` | Public C API compatibility | Keep on public C-header facade. |
| `tests/ffi_panic_audit.rs` | `raw::options::_mi_assert_fail` under `unstable-raw-internals` | Internal panic/abort boundary audit | Gated behind unstable raw internals. |
| `tests/freelist_encoding.rs` | `raw::internal`, `raw::types` under `unstable-raw-internals` | Internal free-list encoding coverage | Gated behind unstable raw internals. |
| `tests/fuzz_alloc_sequences.rs` | `raw::mimalloc::*` | Public C API fuzz-style operation sequences | Keep on public C-header facade. |
| `tests/lifecycle_stats_exports.rs` | root `raw::*` public C-header facade | Public C stats/lifecycle export behavior | Migrated to public C-header facade. |
| `tests/no_allocation_critical_paths.rs` | `raw::mimalloc::*` | Public C API behavior under no-allocation constraints | Keep on public C-header facade. |
| `tests/options_exports.rs` | `raw::mimalloc::*` | Public C option export behavior | Keep on public C-header facade. |
| `tests/original_main.rs` | root `raw::mi_*` public C-header facade | Port of upstream C heap/allocation behavior | Migrated to public C-header facade. |
| `tests/original_main_override.rs` | root `raw::mi_*`, `raw::headers` | Port of upstream C override test plus C++ header placeholders | Public C calls migrated; header placeholders remain compatibility surface. |
| `tests/original_main_override_static.rs` | root `raw::mi_*`, `raw::bits`, `raw::internal`, `raw::page` under `unstable-raw-internals` | Static override regression coverage and internal invariants | Public C calls migrated; internal invariants gated behind unstable raw internals. |
| `tests/original_main_static_dep.rs` | root `raw::mi_malloc`, `raw::mi_free` | Public allocation behavior | Migrated to public C-header facade. |
| `tests/original_test_api.rs` | root `raw::mi_*`, `raw::headers` | Port of upstream API test plus C++ header placeholders | Public C calls migrated; header placeholders remain compatibility surface. |
| `tests/original_test_api_fill.rs` | root `raw::mi_*`, `raw::internal`, `raw::track`, `raw::types` under `unstable-raw-internals` | Debug fill/tracking parity | Public C calls migrated; debug/tracking constants gated behind unstable raw internals. |
| `tests/original_test_stress.rs` | root `raw::mi_*` public C-header facade | Port of upstream stress test | Migrated to public C-header facade. |
| `tests/property_alloc_sequences.rs` | `raw::mimalloc::*` | Public C API property sequences | Keep on public C-header facade. |
| `tests/release_fast_path_audit.rs` | `raw::atomic`, `raw::types` under `unstable-raw-internals` | Source-shape and layout audit helpers | Gated behind unstable raw internals. |
| `tests/secure_features.rs` | `raw::internal`, `raw::types` under `unstable-raw-internals` | Secure feature constants/types | Gated behind unstable raw internals. |
| `tests/stats_exports.rs` | root `raw::mi_stats_get_bin_size`, `raw::MI_BIN_HUGE` | Public stats export behavior | Migrated to public C-header facade. |
| `tests/thread_stats_regression.rs` | root `raw::mi_*`, `raw::mi_stats_t`, `raw::MI_STAT_VERSION` | Public stats regression behavior | Migrated to public C-header facade. |
| `tests/tracking_features.rs` | `raw::internal`, `raw::track` under `unstable-raw-internals` | Tracking feature constants | Gated behind unstable raw internals. |

## Raw Module Classification

| Raw module | Classification | Default-facade target |
|---|---|---|
| `raw::abi` | Public C header surface | Keep. |
| `raw::mimalloc` | Public C header surface alias | Keep. |
| `raw::headers` | C/C++ header macro compatibility placeholders | Keep until those placeholders are folded into `raw::abi` or removed. |
| `raw::types` | Public C layout aliases mixed with internal implementation types | Gated; C-public layouts/types are re-exported at raw root through `raw::abi`. |
| `raw::alloc` | Public C ABI implementation/testing convenience mixed with internals | Gated; public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::alloc_aligned` | Public C ABI implementation/testing convenience mixed with internals | Gated; public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::alloc_posix` | Public C ABI implementation/testing convenience | Gated; public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::alloc_override` | Public C ABI override implementation/testing convenience | Gated by default; C symbols remain dynamic exports. |
| `raw::free` | Public C ABI implementation/testing convenience mixed with internals | Gated; public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::heap` | Public C heap API implementation/testing convenience mixed with internals | Gated; public calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::stats` | Public C stats layouts/functions mixed with internals | Gated; C-public stats types/functions are re-exported at raw root through `raw::abi`. |
| `raw::options` | Public C option API mixed with internals | Gated; C-public option types/functions are re-exported at raw root through `raw::abi`. |
| `raw::track` | Tracking feature constants/hooks | Gated under `unstable-raw-internals`. |
| `raw::internal` | Internal helper required by tests only | Gated under `unstable-raw-internals`. |
| `raw::page` | Internal allocator implementation | Gated under `unstable-raw-internals`. |
| `raw::page_map` | Internal allocator implementation | Gated under `unstable-raw-internals`. |
| `raw::theap` | Internal/thread-heap implementation mixed with C helper surface | Gated; public C calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::arena` | Internal arena implementation mixed with public arena exports | Gated; public C calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::arena_meta` | Internal arena metadata | Gated under `unstable-raw-internals`. |
| `raw::os` | Internal OS facade with selected exported helpers | Gated; exported C symbols remain dynamic exports. |
| `raw::prim` | Platform primitive internals | Gated under `unstable-raw-internals`. |
| `raw::threadlocal` | Internal thread-local implementation | Gated under `unstable-raw-internals`. |
| `raw::random` | Internal random/hardening implementation | Gated under `unstable-raw-internals`. |
| `raw::memory` | Internal memory helpers | Gated under `unstable-raw-internals`. |
| `raw::bitmap` | Internal bitmap helpers | Gated under `unstable-raw-internals`. |
| `raw::bits` | Internal bit helpers | Gated under `unstable-raw-internals`. |
| `raw::libc` | Internal libc compatibility helpers | Gated under `unstable-raw-internals`. |
| `raw::init` | Internal process/thread initialization mixed with public lifecycle API | Gated; public lifecycle calls use root `raw::mi_*`/`raw::mimalloc`. |
| `raw::static_c` | Internal static facade compatibility | Gated under `unstable-raw-internals`. |

## Migration Order

1. Move remaining `raw::mimalloc::*` imports to root `raw::mi_*` imports where
   that improves readability.
2. Move tests that assert internal invariants into crate-private unit tests
   where possible.
3. Remove `unstable-raw-internals` once no integration test or repository audit
   requires internal module paths.
