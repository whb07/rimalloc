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

### Fixed

- Enable native macOS DYLD interposition with an SDK-defined malloc zone,
  ownership-aware system allocator fallback, correct errno and alignment
  behavior, and libc++ new/delete semantics. Add C/C++ injection tests for
  ordinary and secure/guarded builds.
- Promote the macOS zone to libmalloc's default zone so statically linked
  `interpose` programs, in every build profile, are overridden too; dyld
  ignores `__interpose` tuples outside dylibs loaded at launch.
- Serve interposed macOS `malloc`, `calloc`, `realloc`, `strdup`, and `strndup`
  from 16-byte aligned plain size classes. The aligned-allocation fallback
  doubled 1-15 byte blocks and grew power-of-two blocks above 4 KiB by 25%, and
  the string copies could be 8-byte aligned.
- Report `malloc_size` and zone ownership only for allocation starts, and free
  owned pointers with a single page-map lookup.
- Keep the macOS default and cached thread heaps and the thread id in Darwin TSD
  slots, as upstream does, instead of Rust thread locals whose first access
  allocates through libmalloc.
- Run process teardown at exit on Apple targets through `atexit`, so
  `MIMALLOC_SHOW_STATS` and `destroy_on_exit` work there.
- Build C shims for Cargo's macOS target architecture and rustc's deployment
  target instead of the host's.
- Leave `errno` unchanged in allocator OS memory calls, which `free` reaches.
- Preserve requested alignment for small allocations in the safe heap API.
- Stop exporting libc and C++ allocator names on Apple targets. Without
  `interpose`, a binary's own `free` leaked libSystem buffers and its `realloc`
  of them returned NULL.
- Pass the dynamic-library crate type to Cargo so macOS builds use the correct
  LTO mode.

- Initialize the allocator through a Mach-O constructor on macOS, avoiding
  bootstrap allocator exhaustion before the first explicit allocation call.
- Use Darwin memory-query and errno constants, the native `timeval` field
  width, byte units for peak RSS, and reusable-page decommit/reuse advice.
- Preserve requested Rust `Layout` alignment for small allocations and
  reallocations in release builds without debug padding.
- Exclude Linux THP accounting on other platforms and skip the Linux-only
  shared-library comparison benchmark on macOS.

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

- Repair the benchmark evidence layer with preallocated timed-region scratch,
  persistent remote-free topologies, explicit public/raw facade labels,
  fresh-process resource records, provenance manifests, and a reviewed
  artifact-aware median/p95 baseline.
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
