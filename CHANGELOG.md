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

### Deprecated

- Treat broad implementation-module raw paths such as `raw::alloc`,
  `raw::heap`, `raw::internal`, `raw::page`, and `raw::types` as unstable
  migration paths, not supported integration paths.

### Removed

- Remove broad implementation-module exposure from the default `raw` facade by
  gating those modules behind `unstable-raw-internals`.
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

### Performance

- No allocator hotpath behavior is intentionally changed by the safe API/raw
  facade split.
- Benchmark and profiling policy is documented in `docs/performance.md` and
  `BENCHMARK_RESULTS.md`.

### Documentation

- Add `docs/safe-api.md`, `docs/abi.md`, `docs/api-inventory.md`,
  `docs/unsafe-audit.md`, and `docs/raw-facade-migration.md` coverage for the
  safe API/raw facade split.
