# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`c500ffb`. The last pass migrated arena, page-queue, heap, and free-list typed
wrappers off the unsafe migration allowlist, narrowed the free-list strict-type
entries, and split the datatype acceptance release-audit wall into a dedicated
module.

The next pass should keep the same discipline: retire one measurable exception
at a time, preserve C ABI and allocator hot-path behavior, and replace
translated-style file allowances with local Rust proof markers, focused
strict-type entries, or smaller review units.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 34 entries: 31 production and
  3 translation files. `ALLOWLIST_ENTRY_CAP` is 34.
- `src/types/memid.rs` now uses 4
  `SAFETY[AUDIT:typed-memid-view]` markers for discriminant-gated union reads,
  plus a `SAFETY[AUDIT:typed-memid-test-fixtures]` marker for its lint-touch
  fixture, and is off the unsafe migration allowlist.
- `src/types/layout.rs` now uses 16
  `SAFETY[AUDIT:typed-layout-domain]` markers for unchecked layout/product,
  old-allocation, and flag constructors, plus 2
  `SAFETY[AUDIT:typed-layout-test-fixtures]` markers for lint-touch fixtures,
  and is off the unsafe migration allowlist.
- The remaining low-risk typed-wrapper migration targets are
  `src/types/page_states.rs` at 473 lines with 60 `unsafe` hits and 0 local
  markers, and `src/types/atomic.rs` at 755 lines with 101 `unsafe` hits and
  0 local markers.
- The smallest remaining allowlisted production files are:
  `src/allocator/arena_meta.rs` at 316 lines with 24 `unsafe` hits and 0 local
  markers,
  `src/raw/posix.rs` at 329 lines with 81 `unsafe` hits and 0 local markers,
  `src/allocator/bench_diagnostics.rs` at 350 lines with 8 `unsafe` hits and
  0 local markers,
  `src/platform/wasi.rs` at 373 lines with 34 `unsafe` hits and 0 local
  markers, and
  `src/lib.rs` at 398 lines with 23 `unsafe` hits and 8 local markers.
- `strict-type-allowlist.txt` has 75 path-wide entries with `symbol = *`,
  including 29 path-wide raw-pointer entries. `REMOVE_ENTRY_CAP` remains 0.
- The largest unsafe audit marker clusters remain broad enough to revisit:
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` has 88 markers,
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 58 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 56 markers,
  `SAFETY[AUDIT:arena-debug-render]` has 35 markers,
  `SAFETY[AUDIT:owned-allocation]` has 32 markers,
  `SAFETY[AUDIT:typed-scalar-test-fixtures]` has 32 markers,
  `SAFETY[AUDIT:unsafe-callback]` has 28 markers,
  `SAFETY[AUDIT:typed-freelist-wrappers]` has 25 markers, and
  `SAFETY[AUDIT:arena-id-memid-helpers]` has 25 markers.
- The largest production review units remain translated-style modules:
  `src/allocator/arena.rs` is 3,915 lines,
  `src/allocator/page.rs` is 2,801 lines,
  `src/allocator/alloc.rs` is 2,559 lines,
  `src/internal/bitmap.rs` is 2,358 lines,
  `src/allocator/init.rs` is 2,339 lines,
  `src/raw/macros.rs` is 2,223 lines, and
  `src/platform/windows.rs` is 2,191 lines.
- The release audit is better split, but still has large review units:
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,384 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,126 lines,
  `tests/release_fast_path_audit.rs` is 1,832 lines, and
  `tests/release_fast_path_audit/datatype_acceptance.rs` is 1,103 lines.

## Issue: MemId typed views still use a file-level unsafe allowance

> **Comment:** `src/types/memid.rs` is already a typed view over the raw
> `mi_memid_t` union. The remaining unsafe operations are the discriminant-gated
> union reads and one dead-code test fixture, so this file should not need a
> broad migration exception.

- [x] Add local `SAFETY[AUDIT:typed-memid-view]` markers for the `os`, `meta`,
      and `arena` union arm reads in `MemId::view`.
- [x] Keep corrupt or unknown discriminants mapped to `MemIdView::Unknown`
      rather than panicking, allocating, or assuming valid union payloads.
- [x] Cover the remaining test-fixture-only unsafe constructor with an existing
      or new fixture audit marker, without expanding production unsafe scope.
- [x] Add matching `UNSAFE_AUDIT.md` and
      `docs/unsafe-audit-evidence.tsv` entries, remove `src/types/memid.rs`
      from `docs/unsafe-migration-allowlist.tsv`, regenerate unsafe inventory,
      and lower `ALLOWLIST_ENTRY_CAP`.

Verify: `cargo test types::memid::tests --lib --all-features`, unsafe audit
scripts, unsafe inventory regeneration/check, release audit, full clippy, and
full tests.

## Issue: Layout request wrappers still use a file-level unsafe allowance

> **Comment:** `src/types/layout.rs` owns allocation size, product, realloc,
> and packed generic-flag domains. Its unsafe operations are unchecked
> constructor boundaries after local validation, not arbitrary translated C
> logic.

- [x] Add local `SAFETY[AUDIT:typed-layout-domain]` markers for unchecked
      effective-size, maybe-alignment, padded-layout, checked-product,
      validated old-allocation, and generic flag construction.
- [x] Preserve the current small `Copy` request/value shape, `#[inline]`
      boundaries, and allocation-free hot-path behavior.
- [x] Keep checked public constructors returning `LayoutError`; do not replace
      ordinary validation with panics or unchecked casts.
- [x] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: `cargo test types::layout::tests --lib --all-features`, unsafe audit
scripts, unsafe inventory regeneration/check, release audit, full clippy, and
full tests.

## Issue: Page-state witnesses still use a file-level unsafe allowance

> **Comment:** `src/types/page_states.rs` encodes local, foreign, abandoned,
> active, available, full, retired, and destroyed page states. It is exactly
> the kind of Rust domain boundary we want, but its witness constructors and
> transitions still hide behind a file-level migration allowance.

- [ ] Add local `SAFETY[AUDIT:typed-page-state-domain]` markers for page and
      thread-heap witness construction, free-list state transitions, abandoned
      ownership checks, and destroy/retire transitions.
- [ ] Keep page-state witnesses transparent and `Copy` where they are today;
      do not add allocation, dynamic dispatch, or duplicate page traversal.
- [ ] Preserve existing tests for abandon/reclaim, all-free transitions,
      available/free block updates, and abandoned mapped classification.
- [ ] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: `cargo test types::page_states::tests --lib --all-features`, unsafe
audit scripts, unsafe inventory regeneration/check, release audit, full clippy,
and full tests.

## Issue: Benchmark diagnostics still carry a file-level unsafe allowance

> **Comment:** `src/allocator/bench_diagnostics.rs` is a small diagnostic C ABI
> surface. Its unsafe operations are export attributes plus caller-provided
> snapshot and JSON buffer writes; they can be documented locally without
> changing counter hot paths.

- [ ] Add local `SAFETY[AUDIT:bench-diagnostics-ffi]` markers for exported
      diagnostic symbols, nullable snapshot writes, JSON buffer NUL
      termination, and bounded byte copies.
- [ ] Preserve relaxed atomic counter updates and avoid heap allocation in the
      diagnostic JSON writer.
- [ ] Add focused tests or release-audit assertions for null buffers,
      truncation/NUL termination, and snapshot copying.
- [ ] Add matching unsafe audit/evidence entries, remove the file from the
      unsafe migration allowlist, regenerate unsafe inventory, and lower the
      cap.

Verify: targeted bench diagnostics tests or release audit, unsafe audit
scripts, unsafe inventory regeneration/check, release audit, full clippy, and
full tests.

## Suggested implementation order

1. [x] Migrate `src/types/memid.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/types/layout.rs` off the unsafe migration allowlist.
3. [ ] Migrate `src/types/page_states.rs` off the unsafe migration allowlist.
4. [ ] Migrate `src/allocator/bench_diagnostics.rs` off the unsafe migration
       allowlist.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist cap is lowered after each migrated file.
- [ ] `src/types/memid.rs`, `src/types/layout.rs`,
      `src/types/page_states.rs`, and `src/allocator/bench_diagnostics.rs` are
      covered by operation-level audit markers and evidence instead of
      file-level migration exceptions.
- [ ] Existing typed-domain behavior, C ABI behavior, and allocation-free hot
      paths remain unchanged.
- [ ] Release-audit TODO guards track the active backlog and measurable
      evidence.
