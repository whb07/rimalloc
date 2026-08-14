# Architecture

This crate has a small Rust-facing facade over a mostly translated mimalloc
backend. The boundary goal is to make public API commitments explicit while
keeping translated allocator code close enough to upstream to audit.

## Layers

- Tier S, safe Rust facade: `src/safe/` plus the curated crate-root re-exports
  in `src/lib.rs`. This layer owns Rust-native ownership, lifetime, error, and
  snapshot types such as `Allocation`, `Heap`, `Options`, and `StatsSnapshot`.
- Tier U, unsafe Rust conveniences: `src/ffi/callbacks.rs`, re-exported as
  `unsafe_callbacks`. These are public unsafe Rust APIs, not safe facade APIs
  and not C-shaped raw APIs.
- Tier R, raw Rust facade: `src/raw/` and the public C-header mirror in
  `src/ffi/abi.rs` and `src/ffi/headers.rs`. This layer keeps C-shaped names,
  raw pointers, constants, layouts, and compatibility macros visible for tests,
  benchmarks, and low-level integrations.
- Tier C, exported C ABI: exported symbols implemented through `src/ffi/`,
  translated raw modules, explicit interposition shims, and build-mode glue.
  Dynamic exports are validated separately from Rust module visibility.
- Tier I, internal implementation: `src/allocator/`, `src/types/`,
  `src/internal/`, and `src/platform/`. These modules carry allocator state,
  translated helpers, strict wrapper types, platform calls, atomics, and
  provenance-sensitive code. They are not public API.
- Build glue: `build.rs` owns C ABI and interposition build setup, including
  optional C/C++ shims, TLS shims, export scripts, and linker flags.
- Policy gates: `scripts/` and focused tests enforce public boundaries,
  ABI/layout drift, feature behavior, concurrency protocols, and behavioral
  parity.

## Boundary Rules

Tier S may call Tier I only through reviewed backend contacts that enforce
safe ownership, lifetime, nullability, initialization, and error mapping at the
safe boundary. `scripts/check-layer-boundaries.py` rejects imports outside the
small set of backend API roots used by the safe facade; it does not mirror each
safe source file or its exact imports in a ledger.

Tier U must stay under the `unsafe_callbacks` namespace. Callback guards are
process-global and unsafe to construct because synchronization, callback
lifetime, allocator reentrancy, and cross-thread entry cannot be proven by the
safe facade.

Tier R and Tier C keep C compatibility spelling and behavior. Raw macros expand
through public `raw::mi_*` paths and require caller-side unsafe blocks. New raw
modules, exported symbols, layout types, or macro exports must be classified in
`docs/api-inventory.md` and covered by the relevant scripts.

Tier I modules may preserve mechanical C shapes when that improves upstream
parity, but new Rust-facing helper code should prefer typed wrappers, explicit
error states, narrow imports, and small unsafe scopes.

## Maintainer Map

Start with these owners instead of searching the whole tree:

| Concern | Owning code |
|---|---|
| Safe Rust ownership, errors, options, and snapshots | `src/safe/`; curated exports in `src/lib.rs` |
| Raw Rust facade and C-header compatibility | `src/raw/`, `src/ffi/abi.rs`, and `src/ffi/headers.rs` |
| Allocation and reallocation | `src/allocator/alloc.rs`; aligned requests in `src/allocator/aligned.rs` |
| Free routing | `src/allocator/free.rs`; abandoned-page routing in `src/allocator/free/abandoned.rs` |
| Page lifecycle and queues | `src/allocator/page.rs` and its `page/` children |
| Arena lifecycle and OS-backed regions | `src/allocator/arena.rs` and its `arena/` children, especially `metadata.rs`, `os_memory.rs`, `purge.rs`, and `abandoned.rs` |
| Process, thread, TLS, and subprocess initialization | `src/allocator/init.rs` orchestrates; `init/cpu.rs`, `init/thread.rs`, `init/tls.rs`, `init/subproc.rs`, and `init/subproc_registry.rs` own the component lifecycles |
| Options and diagnostics policy | `src/allocator/options.rs` orchestrates; `options/storage.rs` owns option state and `options/diagnostics.rs` owns output/callback handling; Rust facade in `src/safe/options.rs` |
| Statistics mutation, aggregation, and rendering | `src/allocator/stats.rs` owns ABI/orchestration; `stats/counters.rs`, `stats/process.rs`, and `stats/render.rs` own their operation families; Rust snapshots in `src/safe/stats.rs` |
| Maintained Linux platform primitives | `src/platform/os.rs`, dispatched through `src/platform/mod.rs` and `src/platform/unix.rs` |

Public boundary detail lives in the [API inventory](docs/api-inventory.md),
hot-path evidence rules live in the [performance policy](docs/performance.md),
and target status lives in the [platform policy](docs/platforms.md).
