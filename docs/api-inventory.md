# Public API Inventory

This inventory separates stable-intent surfaces from compatibility surfaces and
implementation internals.

## Crate Root

- `MiMalloc`: Rust `GlobalAlloc` facade.
- `Allocation`: owned byte allocation wrapper backed by `MiMalloc`.
- `AllocError`, `LayoutError`, `TryAllocError`: Rust-native error/status types
  for the safe allocation wrapper.
- `raw`: explicit C-shaped compatibility facade.

Implementation modules such as allocation, page, heap, arena, OS, primitive,
thread-local, and strict-type internals are crate-private and must not be
re-exported from the crate root.

## Safe Rust API

`MiMalloc` implements `core::alloc::GlobalAlloc`. `Allocation` owns one byte
allocation and releases it on drop, translating C-style null allocation failure
into Rust `Result` values.

No safe Rust `Heap`, `StatsSnapshot`, `Options`, or allocator-trait wrapper is
stabilized yet.

Adding a safe wrapper requires documenting how it enforces ownership, lifetime,
aliasing, nullability, alignment, threading, and unwind requirements without
caller discipline.

## Raw Rust Facade

`mechanical_mimalloc::raw` exposes C-shaped functions, constants, callbacks,
and `repr(C)` layouts for tests, benchmarks, and low-level integration.

The raw facade is a compatibility surface, not a safe abstraction. Most calls
are unsafe because they inherit mimalloc's C preconditions.

The source/documentation inventory is checked by:

```sh
scripts/check-api-inventory.py
```

## C ABI Symbols

The `cdylib` exports mimalloc-compatible C ABI symbols. The authoritative local
checks are:

```sh
scripts/check-abi-symbols.sh
scripts/check-layout.sh
```

Extra exported symbols outside the public C headers must be classified in
`symbols/public-symbol-classification.tsv`; otherwise the symbol check fails.

## Hidden Internals

Internal C-shaped helper functions may remain `extern "C"` when translated code
depends on that shape, but they must not become dynamic ABI exports unless the
symbol is deliberately classified. `tests/release_fast_path_audit.rs` guards
known internal helpers against regaining `#[unsafe(no_mangle)]`.

## SemVer Classification

- Crate root safe API: Rust SemVer surface.
- `raw`: unstable low-level Rust path surface until explicitly versioned.
- C ABI symbols and layouts: C compatibility surface.
- `core_types`: private design surface, not public SemVer.
- Benchmarks, scripts, and docs: release-process surface, not allocator API.

Detailed compatibility domains and release classification rules are documented
in `docs/compatibility.md`.
