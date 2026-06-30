# Migration Guide from C mimalloc

This guide maps common C mimalloc integration patterns to the supported
mechanical_mimalloc Rust and FFI surfaces.

Use `docs/compatibility.md` for compatibility tiers, `docs/abi.md` for C ABI
layout/callback rules, `docs/ffi-linking.md` for release artifacts, and
`docs/symbol-versioning.md` for symbol policy.

## Header to Artifact Map

The crates.io package is the Rust library distribution. C headers and linkable
C ABI artifacts are release assets built from the tagged repository checkout
with `scripts/build-release-artifacts.py`.

| C mimalloc header | mechanical_mimalloc release artifact path |
| --- | --- |
| `mimalloc.h` | Core C ABI declarations for `c-abi-cdylib`, `interpose-cdylib`, and preview static artifacts. |
| `mimalloc-stats.h` | Stats declarations shipped with the same C ABI artifacts. |
| `mimalloc-override.h` | Interposition declarations for `interpose-cdylib` and `interpose-static-preview`. |
| `mimalloc-new-delete.h` | C++ operator new/delete override declarations for interpose artifacts. |

Use `c-abi-cdylib` for explicit `mi_*` calls without replacing the process
allocator. Use `interpose-cdylib` for `LD_PRELOAD`, malloc-family override, and
C++ operator override scenarios. Static archives are preview-only.

## Allocation and Free APIs

Choose the Rust surface by ownership model, not by C function spelling.

| C pattern | Rust or FFI choice |
| --- | --- |
| Process-wide allocator replacement | `#[global_allocator] static GLOBAL: MiMalloc = MiMalloc;` |
| `mi_malloc`, `mi_zalloc`, `mi_calloc`, `mi_realloc`, `mi_free` for Rust-owned bytes | `Allocation` or `InitializedAllocation`, which free on drop and return typed errors. |
| Heap-scoped `mi_heap_*` allocation with Rust-owned bytes | `Heap`, `HeapRef`, `MainHeap`, `HeapAllocation`, or `InitializedHeapAllocation`. |
| Exact C pointer behavior, null returns, or ownership crossing a C boundary | `mechanical_mimalloc::raw` or the release C ABI artifact. |
| C macro compatibility | `raw::headers` with the `c-header-macros` feature when the macro is intentionally part of the raw header mirror. |

Prefer safe Rust allocation wrappers when the allocation is owned by Rust code.
They encode layout, initialization, and drop behavior directly and avoid raw
pointer ownership transfer. Use the raw facade only when the caller can uphold
the same pointer provenance, allocation-origin, alignment, lifetime,
nullability, and threading rules as the C API.

## Callback Migration

C callback registration remains process-global. The raw callback declarations
mirror upstream C signatures and nullable callback slots in `docs/abi.md`.
Callbacks must not unwind across the C ABI boundary.

`mechanical_mimalloc::unsafe_callbacks` currently provides Tier U borrowed Rust
guards for:

- `unsafe_callbacks::ErrorCallbackGuard` over `mi_register_error`;
- `unsafe_callbacks::DeferredFreeCallbackGuard` over
  `mi_register_deferred_free`.

These guards restore the previous registration on drop and catch Rust panics
before returning through the callback ABI in `std` and test builds. Their
constructors are unsafe because callers must serialize registration,
restoration, and callback entry for the borrowed closure.
Nested guards must be dropped in LIFO order.

Output callbacks, heap visitors, block visitors, and managed-memory commit
callbacks remain raw C-shaped APIs until their recursion, traversal lifetime,
and process-global behavior can be modeled as a Rust API.

## Options and Statistics

Use safe wrappers for supported non-pointer operations:

| C API family | Safe Rust choice | Raw/FFI choice |
| --- | --- | --- |
| `mi_option_get`, `mi_option_set`, related option APIs | `GlobalOptions` and `OptionKey` with `OptionError` for invalid, immutable, or unsupported options. | `mechanical_mimalloc::raw` or the C ABI artifact for exact C option values and null/error behavior. |
| `mi_stats_*` non-printing snapshots | `StatsSnapshot`, `StatCount`, and `StatTotal`. | `mechanical_mimalloc::raw` or the C ABI artifact for printing callbacks and exact C stats APIs. |
| `mi_process_info` | `ProcessInfo`, which maps unavailable size totals to `None`. | Raw C API when C out-pointer behavior is required. |

Safe stats wrappers intentionally avoid exposing raw allocator metadata pointers
or printing callback lifetimes.

## Unsupported Platform and Feature Differences

The maintained release target is Linux x86_64 GNU. Other upstream mimalloc
platform code may be present in the translation, but source presence is not a
maintained platform promise. See `docs/platforms.md` and `docs/limitations.md`
before relying on a non-maintained platform.

Feature choices differ from upstream C build flags:

- default features are empty;
- `std` is for tooling, tests, examples, and benchmarks;
- `secure`, `guarded`, `track-valgrind`, and `track-asan` are explicit
  hardening/tracking feature modes;
- `c-abi` prepares C ABI artifacts;
- `cxx-overrides` adds the C++ operator shim and implies `c-abi`;
- `tls-shim` adds the Linux initial-exec TLS shim and implies `c-abi`;
- `interpose` is the aggregate release interposition mode and implies
  `c-abi`, `cxx-overrides`, and `tls-shim`.

Do not infer a stable C ABI, static archive contract, C++ override contract, or
interposition contract for unsupported platforms. Static archives are
preview-only until the static symbol contract, collision tests, and checked
static manifests are complete.
