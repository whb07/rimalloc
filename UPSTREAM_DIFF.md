# Upstream Diff Notes

Baseline: mimalloc 3.3.2, with the vendored C baseline in
`mimalloc-3.3.2/`.

This file records deliberate local differences from upstream mimalloc and
current limitations that must be reconsidered during an upstream sync.
`docs/upstream.md` defines the sync policy; `PARITY.md` records behavioral
parity evidence and known C ABI divergences.

## Deliberate Local Differences

- The repository builds both a Rust `rlib` and a C-compatible `cdylib`.
  Upstream mimalloc is a C/C++ allocator library.
- The crate root exposes a small safe Rust facade (`MiMalloc`, `Allocation`,
  and allocation error types) plus an explicit `raw` facade. Implementation
  modules are private so the translation graph is not a Rust SemVer contract.
- `src/abi.rs` owns raw C header-shaped declarations used by the Rust
  translation. `src/mimalloc.rs` is an internal translation alias rather than
  the owner of public C ABI declarations.
- Linux x86_64 GNU builds use local support shims and audits for C ABI parity,
  including TLS storage, recursion guard storage, C++ operator new/delete
  overrides, global assembly, and a Linux version script.
- `src/core_types/` contains Rust-only stricter internal types for allocator
  domains, scalar layout, atomic wrappers, slices, and ownership boundaries.
  These are implementation architecture, not upstream C API.
- The safe Rust API is intentionally limited while heap, stats, option, and
  ownership invariants are still being audited. See `docs/api-inventory.md` and
  `docs/limitations.md`.
- Criterion benchmark ports live in `benches/` and compare translated Rust
  workloads with the vendored C mimalloc 3.3.2 library. Redis, RocksDB, and
  espresso-style external application benchmarks are not part of the current
  required Rust-port benchmark set.
- Local scripts provide release-shape, source-shape, ABI, layout, feature,
  package, and behavioral parity audits. These are repository release controls,
  not upstream mimalloc artifacts.

## Current Limitations To Recheck

- The maintained target is Linux x86_64 GNU. Other upstream platform code is
  source-present but not release-supported without fresh platform evidence.
- Miri coverage uses pure model tests for translated invariants; running the
  full allocator under Miri is not currently expected to work.
- Fuzz-style coverage is deterministic and in-tree. A maintained external
  libFuzzer or `cargo fuzz` target has not been promoted to release evidence.
- Benchmark reports identify noisy scenarios separately. Current watch items
  are documented in `BENCHMARK_RESULTS.md` and should not be treated as
  confirmed regressions without reruns.

## Upstream Sync Checklist

When moving from mimalloc 3.3.2 to a newer upstream baseline:

1. Update the baseline version, tag, or commit recorded here and in
   `docs/upstream.md`.
2. Recompare public headers, exported symbols, callbacks, option/stat
   definitions, C++ operator overrides, and feature-dependent layouts.
3. Refresh `symbols/public-symbol-classification.tsv` for any Rust-only or
   compatibility helper symbols whose visibility changed.
4. Re-run ABI, layout, C-link, behavioral parity, feature sweep, docs,
   package, and release fast-path audits.
5. Re-run targeted benchmark smoke tests for touched workloads and a full
   benchmark report when hot paths or allocator-wide behavior changed.
6. Update `PARITY.md`, `BENCHMARK_RESULTS.md`, `docs/abi.md`,
   `docs/platforms.md`, `docs/performance.md`, and `docs/limitations.md` for
   any changed guarantees, deviations, or unsupported cases.
