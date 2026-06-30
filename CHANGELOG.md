# Changelog

All notable changes to this repository should be recorded here.

## Unreleased

### Added

- Define the crate as a dual C ABI allocator artifact and experimental Rust
  library facade.
- Add the narrow safe Rust facade: `MiMalloc`, `Allocation`, `AllocError`,
  `LayoutError`, and `TryAllocError`.
- Add the explicit `raw` compatibility facade for C-shaped Rust access.
- Add package metadata, project documentation, and initial policy documents.

### Changed

- Make implementation modules private at the crate root so translated internal
  structure does not become accidental Rust API.
- Narrow the default `raw` facade to public C-header-compatible paths:
  `raw::mi_*`, `raw::abi`, `raw::mimalloc`, and `raw::headers`.
- Keep the public `raw` module as an unsafe, Rust-path-unstable Tier R facade
  for C-header parity, repository tests, benchmarks, and low-level integrations.

### Removed

- Remove broad implementation-module exposure from the `raw` facade. Use root
  `raw::mi_*`, `raw::abi`, `raw::mimalloc`, or `raw::headers` for supported raw
  C-header compatibility paths.
- Remove the pre-release `Options` compatibility alias. Use `GlobalOptions` for
  supported process-global allocator option access.
- Remove the pre-release `Allocation::into_initialized` and
  `InitializedAllocation::into_initialized` methods. `Allocation::zeroed` and
  `Allocation::zeroed_with_size_align` now return `InitializedAllocation`
  directly, and callers that initialize uninitialized allocation bytes through
  raw pointers should use unsafe `Allocation::assume_initialized`.

### Safety

- Document the safe API boundary, raw caller obligations, FFI unwind boundary,
  callback obligations, and deferred safe heap/stats/options wrappers.
- Add safe facade tests for `MiMalloc` global allocation, `Allocation`, raw
  pointer escape hatches, and feature interactions.

### ABI

- Preserve mimalloc-compatible dynamic C ABI symbols and public C layouts.
- Classify the raw Rust facade separately from the dynamic C ABI; raw Rust path
  cleanup is not a C ABI symbol removal.
- Mark static archive artifacts preview-only until the static symbol contract,
  collision behavior, and checked static manifests are completed.

### Performance

- No allocator hotpath behavior is intentionally changed by the safe API/raw
  facade split.
- Benchmark and profiling policy is documented in `docs/performance.md` and
  `BENCHMARK_RESULTS.md`.
- Release notes must link to `BENCHMARK_RESULTS.md` or an attached performance
  report that summarizes geomean Rust/C ratios, hot-path deltas, noisy watch
  items, release-blocking regressions, and raw Criterion or threshold CSV
  artifacts.

### Documentation

- Add `docs/safe-api.md`, `docs/abi.md`, `docs/api-inventory.md`,
  `docs/unsafe-audit.md`, and `docs/raw-facade-migration.md` coverage for the
  safe API/raw facade split.
