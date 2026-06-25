# TODO: Exemplary Rust Library Readiness

Scope: finish the product, validation, release, portability, safety, performance, and build-system gaps that keep this crate from being an exemplary Rust library.

Do not mark an item done until its acceptance check passes.

## 1. Make The Existing Validation Gates Green

- [x] Fix the `release_fast_path_audit` failure in `scripts/check-features.sh`.
  - Acceptance: `scripts/check-features.sh` exits 0.
  - Check: `cargo test --quiet --features std\ unstable-raw-internals --test release_fast_path_audit free_path_internals_are_not_dynamic_exports` passes.

- [x] Classify or remove the benchmark-diagnostics export boundary violation.
  - Acceptance: `src/bench_diagnostics.rs` no longer appears as an unclassified export file.
  - Check: `scripts/check-ffi-boundary.py` exits 0.

- [x] Verify the raw facade inventory remains stable after the fixes above.
  - Acceptance: default raw facade and unstable raw-internals inventory match the intended public boundary.
  - Check: `scripts/check-api-inventory.py` exits 0.

- [x] Verify safe facade boundary rules after the fixes above.
  - Acceptance: safe public APIs expose no unintended unsafe boundary.
  - Check: `scripts/check-safe-boundary.py` exits 0.

- [x] Verify C ABI boundary rules after the fixes above.
  - Acceptance: exported ABI attributes are limited to classified boundary files.
  - Check: `scripts/check-ffi-boundary.py --list` shows only intended export sites.

- [x] Verify raw facade checks after the fixes above.
  - Acceptance: public raw facade shape remains compatible with the intended C-shaped surface.
  - Check: `scripts/check-raw-facade.sh` exits 0.

- [x] Verify layout compatibility after the fixes above.
  - Acceptance: wrapper and raw-layout compatibility tests pass across the scripted matrix.
  - Check: `scripts/check-layout.sh` exits 0.

- [x] Verify ABI symbol compatibility after the fixes above.
  - Acceptance: expected public ABI symbols are present and extra exports remain classified.
  - Check: `scripts/check-abi-symbols.sh` exits 0.

- [x] Verify strict-type enforcement after the fixes above.
  - Acceptance: remaining raw pointer and bare-domain sites are true boundaries or allowlisted.
  - Check: `scripts/check-strict-types.py` exits 0.

- [x] Verify lint cleanliness after the fixes above.
  - Acceptance: clippy passes all configured target and feature combinations.
  - Check: `scripts/check-lints.sh` exits 0.

## 2. Make Routine Commands Warning-Free And Quiet

- [x] Eliminate warnings from the library-only test build.
  - Acceptance: `cargo test --quiet --lib` prints no Rust compiler warnings.
  - Check: `cargo test --quiet --lib 2>&1 | tee /tmp/mechanical_mimalloc-lib-test.log` contains no line starting with `warning:`.

- [x] Eliminate warnings from the `std` test build.
  - Acceptance: `cargo test --quiet --features std` prints no Rust compiler warnings.
  - Check: `cargo test --quiet --features std 2>&1 | tee /tmp/mechanical_mimalloc-std-test.log` contains no line starting with `warning:`.

- [x] Eliminate warnings from feature-matrix checks.
  - Acceptance: `scripts/check-features.sh` prints no Rust compiler warnings.
  - Check: `scripts/check-features.sh 2>&1 | tee /tmp/mechanical_mimalloc-features.log` contains no line starting with `warning:`.

- [x] Silence expected allocator error output in tests that intentionally trigger invalid frees or huge allocation failures.
  - Acceptance: intentional allocator-error tests still assert behavior without noisy stderr during normal test runs.
  - Check: `cargo test --quiet --features std 2>&1 | tee /tmp/mechanical_mimalloc-std-test.log` contains no unexpected `mimalloc: error:` lines.

- [x] Silence expected stats dumps in tests that intentionally exercise statistics printing.
  - Acceptance: stats-output tests capture or gate their output instead of printing during normal test runs.
  - Check: `cargo test --quiet --features std 2>&1 | tee /tmp/mechanical_mimalloc-std-test.log` contains no unsolicited `subproc`, `blocks`, `pages`, or `arenas` stats tables.

## 3. Finish The Safe Rust Allocation Surface

- [x] Add `ResizeErrorKind`.
  - Acceptance: the error distinguishes invalid layouts from allocator failure.
  - Check: unit tests cover both `ResizeErrorKind::Layout` and `ResizeErrorKind::Alloc`.

- [x] Add `ResizeError<A>` that preserves ownership on resize failure.
  - Acceptance: callers can borrow, mutate, recover, or split the original allocation from the error.
  - Check: tests cover `allocation`, `allocation_mut`, `into_allocation`, and `into_parts`.

- [x] Add `Allocation::resize`.
  - Acceptance: successful resize preserves bytes up to `min(old_len, new_len)` and updates the stored layout.
  - Check: tests cover grow, shrink, same-size, zero-to-nonzero, and nonzero-to-zero resize.

- [x] Add `Allocation::resize_with_size_align`.
  - Acceptance: invalid size/alignment returns a layout error and preserves ownership.
  - Check: tests cover invalid alignment and oversize layout without losing the original allocation.

- [x] Add safe initialized byte-slice access for zeroed allocations.
  - Acceptance: safe slice reads are available only for memory known to be initialized.
  - Check: compile-fail coverage rejects reading an uninitialized `Allocation` as a byte slice.

- [x] Add a type for initialized owned byte allocations.
  - Acceptance: the type can expose `&[u8]` and `&mut [u8]` without exposing uninitialized bytes.
  - Check: tests verify zero initialization and mutable writes through the safe slice API.

- [x] Add conversion from zeroed `Allocation` to initialized allocation.
  - Acceptance: conversion is safe only when initialization is guaranteed.
  - Check: tests verify `Allocation::zeroed(layout)` can become the initialized type.

- [x] Add an unsafe initialization escape hatch for callers that initialize through raw pointers.
  - Acceptance: the API requires an explicit unsafe call and preserves layout metadata.
  - Check: compile-fail coverage rejects the conversion outside an unsafe block.

- [x] Add `Allocation::try_clone_zeroed_contents` or equivalent copy constructor for initialized allocations.
  - Acceptance: cloning initialized bytes either succeeds with identical contents or returns a typed allocation error.
  - Check: tests cover successful copy and ownership preservation on forced failure where practical.

## 4. Add Safe Heap Ownership APIs

- [x] Add `HeapError`.
  - Acceptance: heap creation failure, main-heap unavailability, and unsupported operations are represented distinctly.
  - Check: tests match every `HeapError` variant.

- [x] Add owned `Heap`.
  - Acceptance: `Heap::new` creates a heap or returns `HeapError::Create` without exposing raw heap pointers in safe APIs.
  - Check: tests create and drop a heap without leaking or double-deleting.

- [x] Add borrowed `HeapRef<'a>`.
  - Acceptance: heap-backed allocations can be tied to a heap borrow.
  - Check: compile-fail coverage rejects an allocation outliving the heap reference it is tied to.

- [x] Add `MainHeap` access.
  - Acceptance: callers can allocate through the main heap without owning or deleting it.
  - Check: tests allocate and free through `MainHeap`.

- [x] Add `HeapAllocation<'heap>`.
  - Acceptance: heap allocations cannot outlive their heap and free through the same heap family.
  - Check: compile-fail coverage rejects moving `HeapAllocation<'heap>` beyond the heap lifetime.

- [x] Add `Heap::allocate`.
  - Acceptance: valid layouts return heap-tied allocations or typed allocation errors.
  - Check: tests cover normal, zero-size, and over-aligned layouts.

- [x] Add `Heap::allocate_zeroed`.
  - Acceptance: returned initialized bytes are all zero.
  - Check: tests read the safe initialized byte slice and verify zeroes.

- [x] Add `HeapAllocation::resize`.
  - Acceptance: failed resize preserves the original heap allocation.
  - Check: tests cover grow, shrink, and failure ownership recovery.

- [x] Add heap deletion safeguards.
  - Acceptance: safe code cannot delete a heap while heap-tied allocations are live.
  - Check: compile-fail coverage rejects dropping or deleting through an invalid lifetime pattern.

## 5. Add Safe Options APIs

- [x] Add `OptionError`.
  - Acceptance: invalid keys and unsupported options are represented distinctly.
  - Check: tests match every `OptionError` variant.

- [x] Add `OptionKey`.
  - Acceptance: public safe option keys cannot be constructed from invalid raw values.
  - Check: tests cover every supported key and reject at least one invalid raw value.

- [x] Add `Options::get`.
  - Acceptance: supported options return current values without unsafe calls by the caller.
  - Check: tests compare safe getter output with the raw C-shaped getter for selected options.

- [x] Add `Options::set`.
  - Acceptance: setting supported mutable options succeeds or returns a typed error.
  - Check: tests set a reversible option, read it back, then restore the original value.

- [x] Add `Options::reset` for supported mutable options.
  - Acceptance: reset returns the option to its default or previous supported baseline.
  - Check: tests mutate, reset, and verify the value changed back.

- [x] Enforce immutable option behavior in the safe API.
  - Acceptance: safe mutation of immutable options returns `OptionError::Unsupported`.
  - Check: tests attempt to set a known immutable option and assert the error.

## 6. Add Safe Statistics APIs

- [x] Add `StatsError`.
  - Acceptance: unavailable stats, unavailable heap stats, and buffer-too-small cases are represented distinctly.
  - Check: tests match every `StatsError` variant.

- [x] Add `StatCount`.
  - Acceptance: count values use a typed wrapper instead of bare integers in the safe API.
  - Check: tests construct, compare, and read raw values from `StatCount`.

- [x] Add `StatTotal`.
  - Acceptance: byte totals use a typed wrapper instead of bare integers in the safe API.
  - Check: tests construct, compare, and read raw values from `StatTotal`.

- [x] Add `StatsSnapshot` for process-wide statistics.
  - Acceptance: a snapshot can be captured without exposing raw stats pointers.
  - Check: tests allocate, capture before/after snapshots, and observe nondecreasing total allocation activity where deterministic.

- [x] Add `StatsSnapshot` for heap-specific statistics where supported.
  - Acceptance: safe heap stats return a snapshot or `StatsError::HeapUnavailable`/`Unsupported` equivalent.
  - Check: tests capture stats for a freshly created heap or assert the typed unsupported error.

- [x] Add a non-printing stats capture path for tests.
  - Acceptance: tests can verify stats without writing allocator tables to stderr/stdout.
  - Check: normal `cargo test --quiet --features std` emits no stats table output.

## 7. Add Safe Process Information APIs

- [x] Add `ProcessInfo`.
  - Acceptance: process allocator information is returned as a typed struct without raw out-pointers in safe code.
  - Check: tests call the safe API and verify basic invariants such as nonzero page size where available.

- [x] Represent unavailable process fields explicitly.
  - Acceptance: platform-unavailable fields use `Option` or a typed error instead of sentinel integer values.
  - Check: tests assert unavailable fields are represented without panics.

- [x] Add process-info parity tests.
  - Acceptance: selected safe fields match raw C-shaped process-info outputs on the current platform.
  - Check: integration test compares safe and raw values for stable fields.

## 8. Add Safe Callback Guards

- [x] Add `CallbackError`.
  - Acceptance: callback registration failures and unsupported callbacks are represented distinctly.
  - Check: tests match every `CallbackError` variant.

- [x] Add `ErrorCallbackGuard`.
  - Acceptance: installing the guard registers an error callback and dropping it restores the previous callback state.
  - Check: tests install, trigger a controlled allocator error where practical, drop, and verify restoration.

- [x] Add `DeferredFreeCallbackGuard`.
  - Acceptance: installing the guard registers a deferred-free callback and dropping it restores the previous callback state.
  - Check: tests register and unregister without leaking callback state between tests.

- [x] Prevent Rust panics from crossing callback ABI boundaries.
  - Acceptance: panicking callbacks are caught or converted into abort/error behavior before crossing the boundary.
  - Check: child-process test verifies a panicking callback does not unwind across `extern "C"`.

- [x] Enforce callback lifetime safety.
  - Acceptance: safe guards cannot outlive captured callback state.
  - Check: compile-fail coverage rejects registering a callback that borrows data for too short a lifetime.

## 9. Finalize The Public Rust API Boundary

- [x] Decide the stable crate-root export set.
  - Acceptance: crate root exports only intended safe types plus `MiMalloc` and `raw`.
  - Check: `scripts/check-api-inventory.py` contains and verifies the final expected list.

- [x] Remove accidental public implementation exposure.
  - Acceptance: implementation modules remain private outside the explicitly unstable raw-internals feature.
  - Check: compile-fail coverage rejects importing implementation modules from crate root.

- [x] Add SemVer guard tests for public item names.
  - Acceptance: removing or renaming a stable public item fails a source-shape inventory test.
  - Check: `scripts/check-api-inventory.py` fails when a listed public item is temporarily removed.

- [x] Replace experimental wording in public API status once the safe surface is complete.
  - Acceptance: public crate status no longer describes the safe API as incomplete or experimental.
  - Check: source search finds no `experimental safe Rust allocation facade` phrase in public crate-level text.

- [x] Enable publishing in the manifest when release criteria are met.
  - Acceptance: `Cargo.toml` no longer disables publishing.
  - Check: `cargo package --allow-dirty` succeeds and reports the expected package contents.

- [x] Set the first release-ready version.
  - Acceptance: `Cargo.toml` version reflects the intended release level.
  - Check: `cargo metadata --no-deps --format-version 1` reports the selected version.

## 10. Expand Platform Confidence

- [x] Add a Linux x86_64 validation lane.
  - Acceptance: the full validation set passes on Linux x86_64.
  - Check: recorded CI job runs `cargo test --quiet --features std`, `scripts/check-lints.sh`, `scripts/check-features.sh`, and ABI/layout checks.

- [x] Add a Linux aarch64 validation lane.
  - Acceptance: the core test and layout matrix passes on Linux aarch64.
  - Check: recorded CI job runs `cargo test --quiet --features std` and `scripts/check-layout.sh`.

- [x] Add a macOS validation lane.
  - Acceptance: allocator override and TLS behavior pass on macOS.
  - Check: recorded CI job runs `cargo test --quiet --features std` plus macOS-specific override tests.

- [x] Add a Windows validation lane.
  - Acceptance: Windows allocator and TLS behavior pass on Windows.
  - Check: recorded CI job runs `cargo test --quiet --features std` plus Windows-specific allocation tests.

- [x] Add a no-default-features release build lane.
  - Acceptance: optimized Linux build without `std` compiles and links.
  - Check: `cargo build --release --no-default-features` exits 0 on Linux.

- [x] Add a feature-powerset validation lane for supported feature combinations.
  - Acceptance: all supported non-mutually-exclusive combinations compile or test according to scope.
  - Check: `scripts/check-features.sh` exits 0 and explicitly rejects `track-valgrind` with `track-asan`.

## 11. Add Memory-Safety And Stress Verification

- [x] Add an AddressSanitizer validation lane.
  - Acceptance: sanitizer build completes representative allocation tests without sanitizer findings.
  - Check: ASan job runs allocation, aligned, realloc, heap, and callback tests.

- [x] Add a ThreadSanitizer validation lane where supported.
  - Acceptance: multithreaded allocation tests complete without sanitizer findings or have tracked suppressions.
  - Check: TSan job runs stress and cross-thread free tests.

- [x] Add a Valgrind validation lane where supported.
  - Acceptance: representative tests complete without unexpected invalid reads/writes or leaks.
  - Check: Valgrind job runs selected integration tests with a checked suppression file.

- [x] Add long-running allocation sequence fuzzing.
  - Acceptance: randomized allocate/free/realloc/alignment sequences run for a configured duration without crashes or invariant failures.
  - Check: fuzz job runs for the configured time budget and stores the failing seed on failure.

- [x] Add concurrency stress fuzzing.
  - Acceptance: cross-thread allocation/free/realloc sequences run for a configured duration without crashes or invariant failures.
  - Check: stress job runs with at least one high-contention seed and one many-thread seed.

- [x] Add oversized-allocation stress coverage.
  - Acceptance: oversize requests consistently return null or typed allocation errors without corrupting allocator state.
  - Check: tests run oversize allocation attempts before and after normal allocations.

- [x] Add over-alignment stress coverage.
  - Acceptance: over-aligned allocations satisfy requested alignment and free correctly.
  - Check: property test covers a range of power-of-two alignments above the default alignment.

- [x] Add zero-size behavior stress coverage.
  - Acceptance: C-shaped zero-size APIs and safe zero-size wrappers behave consistently with their contracts.
  - Check: tests cover allocate, free, realloc, and resize involving zero-size layouts.

## 12. Add Performance Regression Gates

- [x] Define benchmark pass/fail thresholds for hot-path allocation.
  - Acceptance: hot-path benchmarks have explicit regression thresholds against a saved baseline.
  - Check: benchmark comparison script exits nonzero when a synthetic regression exceeds the threshold.

- [x] Define benchmark pass/fail thresholds for aligned allocation.
  - Acceptance: aligned allocation benchmarks have explicit regression thresholds against a saved baseline.
  - Check: benchmark comparison script validates current results against the aligned baseline.

- [x] Define benchmark pass/fail thresholds for realloc workloads.
  - Acceptance: realloc benchmarks have explicit regression thresholds against a saved baseline.
  - Check: benchmark comparison script validates current realloc results against the baseline.

- [x] Define benchmark pass/fail thresholds for multithreaded workloads.
  - Acceptance: multithreaded benchmarks have explicit regression thresholds against a saved baseline.
  - Check: benchmark comparison script validates current multithreaded results against the baseline.

- [x] Add upstream mimalloc comparison gates.
  - Acceptance: selected benchmarks compare this crate against upstream mimalloc under the same workload parameters.
  - Check: comparison run reports this crate within the chosen tolerance for each gated workload.

- [x] Add system allocator comparison gates.
  - Acceptance: selected benchmarks compare this crate against the system allocator under the same workload parameters.
  - Check: comparison run reports results and fails only on configured regressions.

- [x] Gate performance checks to release builds only.
  - Acceptance: benchmark gates never run against debug builds by accident.
  - Check: benchmark scripts assert `--release` or equivalent optimized settings before measuring.

- [x] Store machine-readable benchmark outputs.
  - Acceptance: benchmark results can be parsed by regression scripts without scraping human output.
  - Check: benchmark run emits JSON or CSV and the parser validates required fields.

## 13. Harden Build And Packaging

- [x] Replace fragile compiler invocations in `build.rs` with explicit failure reporting.
  - Acceptance: missing C/C++ compiler, archiver, or linker support produces actionable errors.
  - Check: tests or dry-run script exercise the failure path with a bogus compiler environment variable.

- [x] Gate C++ new/delete shim builds by target support.
  - Acceptance: unsupported targets skip or error clearly instead of failing later in linking.
  - Check: cross-check job verifies the selected targets either build the shim or report the typed unsupported path.

- [x] Gate TLS shim builds by target support.
  - Acceptance: unsupported TLS targets skip or error clearly instead of failing later in linking.
  - Check: cross-check job verifies supported and unsupported TLS configurations.

- [x] Verify cdylib export control on Linux release builds.
  - Acceptance: the Linux version script exports only intended public patterns.
  - Check: `scripts/check-abi-symbols.sh` exits 0 on a fresh release build.

- [x] Verify non-Linux export behavior.
  - Acceptance: macOS and Windows builds expose intended symbols without extra allocator internals.
  - Check: platform-specific symbol inventory scripts pass on macOS and Windows.

- [x] Verify package contents.
  - Acceptance: package includes only required source, tests, examples, benchmarks, scripts, and metadata.
  - Check: `cargo package --allow-dirty --list` matches the intended include set.

- [x] Verify consumer build from packaged crate.
  - Acceptance: a fresh external crate can depend on the packaged artifact and use `MiMalloc` as global allocator.
  - Check: package smoke test builds a temporary consumer crate against the `.crate` file.

## 14. Final Release Signoff

- [x] Run the full local validation set.
  - Acceptance: all required local validation commands exit 0.
  - Check: run `cargo fmt --check`, `cargo test --quiet --lib`, `cargo test --quiet --features std`, `scripts/check-api-inventory.py`, `scripts/check-safe-boundary.py`, `scripts/check-ffi-boundary.py`, `scripts/check-raw-facade.sh`, `scripts/check-layout.sh`, `scripts/check-lints.sh`, `scripts/check-features.sh`, `scripts/check-strict-types.py`, and `scripts/check-abi-symbols.sh`.

- [x] Run the full release-mode validation set.
  - Acceptance: release build and release-focused tests pass with intended feature combinations.
  - Check: `cargo build --release --no-default-features` and release-mode ABI/symbol checks exit 0.

- [x] Run the full safety validation set.
  - Acceptance: sanitizer, Valgrind, fuzz, and stress lanes complete without untriaged failures.
  - Check: each safety lane reports pass or has a linked tracked suppression/issue.

- [x] Run the full performance validation set.
  - Acceptance: gated benchmarks are within configured thresholds.
  - Check: benchmark regression script exits 0 for all gated workloads.

- [x] Run package smoke tests.
  - Acceptance: packaged crate builds and works from a clean consumer project.
  - Check: temporary consumer crate builds, runs a basic allocation, and exits 0.
