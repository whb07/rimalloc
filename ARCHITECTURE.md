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
safe boundary. `scripts/check-layer-boundaries.py` keeps those contacts
file-specific so a new safe facade file cannot import allocator internals
without updating the allowlist and this policy.

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
