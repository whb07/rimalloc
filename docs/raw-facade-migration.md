# Raw Facade Migration Inventory

This inventory tracks current `mechanical_mimalloc::raw` users and classifies
the broad raw modules before narrowing or gating default raw internals.

## Current Raw Users

| Caller | Raw paths | Reason | Migration target |
|---|---|---|---|
| `README.md` | root `raw::mi_malloc`, `raw::mi_free` | Raw usage example | Keep as public C API example. |
| `src/lib.rs` crate docs | root `raw::mi_malloc`, `raw::mi_free` | Raw usage and compile-fail examples | Keep as public C API examples. |
| `src/abi.rs` macros | root `raw::mi_*`, `raw::mi_heap_*` | C header-style allocation helper macros | Keep only if macros remain public C-header compatibility; otherwise move through `raw::abi`/`raw::mimalloc`. |
| `benches/hotpath.rs` | `raw::alloc`, `raw::alloc_aligned`, `raw::free` | Allocator hot-path benchmark shim | Migrate to root `raw::mi_*` C-header functions or safe `MiMalloc` when raw internals narrow. |
| `benches/workloads.rs` | `raw::alloc`, `raw::alloc_aligned`, `raw::free` | Allocator workload benchmark shim | Migrate to root `raw::mi_*` C-header functions or safe `MiMalloc` when raw internals narrow. |
| `examples/aligned.rs` | root `raw::mi_malloc_aligned`, `raw::mi_realloc_aligned`, `raw::mi_free` | Public C API aligned allocation example | Keep as public C API example. |
| `examples/ffi.rs` | root `raw::mi_malloc`, `raw::mi_usable_size`, `raw::mi_free` | Public C API FFI example | Keep as public C API example. |
| `examples/heap.rs` | root heap `raw::mi_heap_*`, `raw::mi_free` | Unsafe C heap API example | Keep as raw public C API example unless a safe heap API is stabilized. |
| `examples/options.rs` | root option functions and `mi_option_t` | Unsafe C option API example | Keep as raw public C API example unless a safe options API is stabilized. |
| `examples/stats.rs` | root stats functions, `mi_stats_t`, `MI_STAT_VERSION` | Unsafe C stats API example | Keep as raw public C API example unless a safe stats snapshot API is stabilized. |
| `tests/aligned_exports.rs` | `raw::mimalloc::*` | Public C API export behavior | Keep on public C-header facade. |
| `tests/bitmap_regressions.rs` | `raw::bitmap` | Internal bitmap regression coverage | Move to crate-private unit tests or gate behind unstable raw internals. |
| `tests/compat_exports.rs` | `raw::mimalloc::*` | Public C API compatibility | Keep on public C-header facade. |
| `tests/ffi_panic_audit.rs` | `raw::options::_mi_assert_fail` | Internal panic/abort boundary audit | Move to crate-private unit test or approved unstable internals. |
| `tests/freelist_encoding.rs` | `raw::internal`, `raw::types` | Internal free-list encoding coverage | Move to crate-private unit tests or gate behind unstable raw internals. |
| `tests/fuzz_alloc_sequences.rs` | `raw::mimalloc::*` | Public C API fuzz-style operation sequences | Keep on public C-header facade. |
| `tests/lifecycle_stats_exports.rs` | `raw::mimalloc::*`, `raw::stats` | Public C stats/lifecycle export behavior | Keep on public C-header facade. |
| `tests/no_allocation_critical_paths.rs` | `raw::mimalloc::*` | Public C API behavior under no-allocation constraints | Keep on public C-header facade. |
| `tests/options_exports.rs` | `raw::mimalloc::*` | Public C option export behavior | Keep on public C-header facade. |
| `tests/original_main.rs` | `raw::alloc`, `raw::free`, `raw::heap` | Port of upstream C test using module internals | Migrate to root `raw::mi_*`/heap public C-header facade. |
| `tests/original_main_override.rs` | `raw::alloc`, `raw::alloc_aligned`, `raw::free`, `raw::headers`, `raw::heap`, `raw::mimalloc`, `raw::options` | Port of upstream C override test | Migrate public API calls to root/`raw::mimalloc`; gate header/internal helpers if still needed. |
| `tests/original_main_override_static.rs` | `raw::bits`, `raw::internal`, `raw::mimalloc`, `raw::options`, `raw::page`, `raw::stats`, `raw::types` | Static override regression coverage and internal invariants | Split public C API checks from crate-private/internal checks. |
| `tests/original_main_static_dep.rs` | `raw::alloc`, `raw::free` | Public allocation behavior | Migrate to root `raw::mi_malloc`/`raw::mi_free`. |
| `tests/original_test_api.rs` | `raw::alloc_aligned`, `raw::free`, `raw::headers`, `raw::options`, `raw::types` | Port of upstream API test | Migrate public API calls to root/`raw::mimalloc`; gate header/type internals if still needed. |
| `tests/original_test_api_fill.rs` | `raw::alloc`, `raw::alloc_aligned`, `raw::free`, `raw::internal`, `raw::options`, `raw::track`, `raw::types` | Debug fill/tracking parity | Split public C API behavior from unstable tracking/internal checks. |
| `tests/original_test_stress.rs` | `raw::alloc`, `raw::free`, `raw::heap`, `raw::mimalloc`, `raw::options` | Port of upstream stress test | Migrate to root/`raw::mimalloc` public C-header facade. |
| `tests/property_alloc_sequences.rs` | `raw::mimalloc::*` | Public C API property sequences | Keep on public C-header facade. |
| `tests/release_fast_path_audit.rs` | `raw::atomic`, `raw::types` | Source-shape and layout audit helpers | Prefer crate-private tests or approved unstable internals. |
| `tests/secure_features.rs` | `raw::internal`, `raw::types` | Secure feature constants/types | Gate behind unstable raw internals or move to crate-private tests. |
| `tests/stats_exports.rs` | `raw::mimalloc`, `raw::stats` | Public stats export behavior | Keep on public C-header facade. |
| `tests/thread_stats_regression.rs` | `raw::mimalloc`, `raw::stats` | Public stats regression behavior | Keep on public C-header facade. |
| `tests/tracking_features.rs` | `raw::internal`, `raw::track` | Tracking feature constants | Gate behind unstable raw internals or move to crate-private tests. |

## Raw Module Classification

| Raw module | Classification | Default-facade target |
|---|---|---|
| `raw::abi` | Public C header surface | Keep. |
| `raw::mimalloc` | Public C header surface alias | Keep or merge into `raw::abi`/root public C API. |
| `raw::types` | Public C layout and alias surface plus compatibility types | Keep only C-public layout/types by default; gate internal-only types later. |
| `raw::alloc` | Public C ABI implementation/testing convenience mixed with internals | Migrate public calls to `raw::mimalloc`; gate internal helpers. |
| `raw::alloc_aligned` | Public C ABI implementation/testing convenience mixed with internals | Migrate public calls to `raw::mimalloc`; gate internal helpers. |
| `raw::alloc_posix` | Public C ABI implementation/testing convenience | Keep public C POSIX allocation surface through `raw::mimalloc`; gate module path if not needed. |
| `raw::alloc_override` | Public C ABI override implementation/testing convenience | Gate by default; expose only C symbols dynamically. |
| `raw::free` | Public C ABI implementation/testing convenience mixed with internals | Migrate public calls to `raw::mimalloc`; gate internal helpers. |
| `raw::heap` | Public C heap API implementation/testing convenience mixed with internals | Keep public C heap functions through `raw::mimalloc`; gate internal helpers. |
| `raw::stats` | Public C stats layouts/functions mixed with internals | Keep public stats types/functions by default; gate internal helpers. |
| `raw::options` | Public C option API mixed with internals | Keep public option types/functions by default; gate internal helpers. |
| `raw::headers` | C header macro compatibility | Keep only if macros remain public; otherwise gate. |
| `raw::track` | Tracking feature constants/hooks | Gate unless a public tracking API is documented. |
| `raw::internal` | Internal helper required by tests only | Gate or remove from default. |
| `raw::page` | Internal allocator implementation | Gate or remove from default. |
| `raw::page_map` | Internal allocator implementation | Gate or remove from default. |
| `raw::theap` | Internal/thread-heap implementation mixed with C helper surface | Gate by default; expose public C calls through `raw::mimalloc`. |
| `raw::arena` | Internal arena implementation mixed with public arena exports | Gate internals; expose public C calls through `raw::mimalloc`. |
| `raw::arena_meta` | Internal arena metadata | Gate or remove from default. |
| `raw::os` | Internal OS facade with selected exported helpers | Gate internals; expose C symbols dynamically only. |
| `raw::prim` | Platform primitive internals | Gate or remove from default. |
| `raw::threadlocal` | Internal thread-local implementation | Gate or remove from default. |
| `raw::random` | Internal random/hardening implementation | Gate or remove from default. |
| `raw::memory` | Internal memory helpers | Gate or remove from default. |
| `raw::bitmap` | Internal bitmap helpers | Gate or move tests crate-private. |
| `raw::bits` | Internal bit helpers | Gate or move tests crate-private. |
| `raw::libc` | Internal libc compatibility helpers | Gate or remove from default. |
| `raw::init` | Internal process/thread initialization mixed with public lifecycle API | Expose public lifecycle calls through `raw::mimalloc`; gate internals. |
| `raw::static_c` | Internal static facade compatibility | Gate or remove from default. |

## Migration Order

1. Move tests and benches that only use public C API behavior to root
   `raw::mi_*` or `raw::mimalloc` imports.
2. Move tests that assert internal invariants into crate-private unit tests
   where possible.
3. Add an explicit `unstable-raw-internals` feature only for remaining external
   integration tests that truly need internal module paths.
4. Narrow default `raw` modules to public C-header surface plus any deliberately
   retained compatibility modules.
