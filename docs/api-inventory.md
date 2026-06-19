# Public API Inventory

This inventory separates stable-intent surfaces from compatibility surfaces and
implementation internals.

## Public API Tiers

- Tier S: safe Rust API, Rust SemVer surface.
- Tier R: raw Rust facade, unsafe and unstable unless explicitly versioned.
- Tier C: exported C ABI and raw layout compatibility surface.
- Tier I: internal implementation surface, not public API.

## Crate Root

- `MiMalloc`: Tier S Rust `GlobalAlloc` facade.
- `Allocation`: Tier S owned byte allocation wrapper backed by `MiMalloc`.
- `AllocError`, `LayoutError`, `TryAllocError`: Tier S Rust-native error/status types
  for the safe allocation wrapper.
- `raw`: Tier R explicit C-shaped compatibility facade.

Implementation modules such as allocation, page, heap, arena, OS, primitive,
thread-local, and strict-type internals are crate-private and must not be
re-exported from the crate root.

## Safe Rust API

`MiMalloc` implements `core::alloc::GlobalAlloc`. `Allocation` owns one byte
allocation and releases it on drop, translating C-style null allocation failure
into Rust `Result` values.

No safe Rust `Heap`, `MainHeap`, `HeapRef`, `HeapAllocation`, `StatsSnapshot`,
`Options`, or allocator-trait wrapper is stabilized yet. Raw heap functions
remain under `raw`; safe heap wrappers are intentionally deferred until their
ownership, lifetime, threading, and deletion semantics are designed.
Safe stats, options, process-info, and callback wrappers are also intentionally
deferred until their snapshot ownership, runtime mutability, recursion, and
platform semantics are designed.

Adding a safe wrapper requires documenting how it enforces ownership, lifetime,
aliasing, nullability, alignment, threading, and unwind requirements without
caller discipline.

Safe API design, naming, and trait rules are documented in `docs/safe-api.md`.

## Safe API Trait Inventory

- `MiMalloc`: implements `core::alloc::GlobalAlloc`; derives `Copy`, `Clone`,
  `Debug`, and `Default`; has auto `Send`/`Sync`; no manual unsafe `Send`/`Sync`
  impls.
- `Allocation`: implements `Drop` and `Debug`; has manual `Send`/`Sync` impls
  documented in `docs/unsafe-audit.md`; intentionally does not implement
  `Copy` or `Clone`.
- `AllocError`: `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Display`, and
  `std::error::Error` when `std` is enabled.
- `LayoutError`: `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Display`, and
  `std::error::Error` when `std` is enabled.
- `TryAllocError`: `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Display`, and
  `std::error::Error` when `std` is enabled.

## Raw Rust Facade

`mechanical_mimalloc::raw` exposes C-shaped functions, constants, callbacks,
and `repr(C)` layouts for tests, benchmarks, and low-level integration.

The raw facade is a compatibility surface, not a safe abstraction. Most calls
are unsafe because they inherit mimalloc's C preconditions.
The raw Rust module path surface is unstable until explicitly versioned.

By default, `raw` exposes the root C-header mirror plus `raw::abi`,
`raw::mimalloc`, and `raw::headers`. Translated implementation modules are
available only with the `unstable-raw-internals` feature for repository audits
and parity tests. That feature is off by default, has no SemVer guarantees, and
must not be used as an application integration surface.

The source/documentation inventory is checked by:

```sh
scripts/check-api-inventory.py
scripts/check-raw-facade.sh
```

Current raw users and raw module classifications are tracked in
`docs/raw-facade-migration.md`.

## docs.rs Behavior

`Cargo.toml` builds docs.rs with the `std` feature so safe error traits and
examples render normally. There are no docs.rs-only public APIs; `docsrs` is
reserved for documentation configuration. Benchmarks are separate Cargo bench
targets and are not presented as normal library APIs in rustdoc.

## C ABI Symbols

The Tier C `cdylib` exports mimalloc-compatible C ABI symbols. The authoritative local
checks are:

```sh
scripts/check-abi-symbols.sh
scripts/check-layout.sh
scripts/check-feature-abi.sh
```

Extra exported symbols outside the public C headers must be classified in
`symbols/public-symbol-classification.tsv`; otherwise the symbol check fails.
`scripts/check-feature-abi.sh` repeats the ABI/layout and export checks for the
layout- or symbol-sensitive `secure`, `guarded`, `track-valgrind`, and
`track-asan` feature sets.

## Hidden Internals

Tier I internal C-shaped helper functions may remain `extern "C"` when translated code
depends on that shape, but they must not become dynamic ABI exports unless the
symbol is deliberately classified. `tests/release_fast_path_audit.rs` guards
known internal helpers against regaining `#[unsafe(no_mangle)]`.

New crate-root public items, new raw modules, or new export attributes outside
approved FFI boundary files are release-blocking until this inventory and the
corresponding source-shape checks are updated.

## SemVer Classification

- Crate root safe API: Rust SemVer surface.
- `raw`: unstable low-level Rust path surface until explicitly versioned.
- C ABI symbols and layouts: C compatibility surface.
- `core_types`: private design surface, not public SemVer.
- Benchmarks, scripts, and docs: release-process surface, not allocator API.

Detailed compatibility domains and release classification rules are documented
in `docs/compatibility.md`.
