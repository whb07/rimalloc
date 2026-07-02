# TODO: Fresh-Eyes Evidence, Source-Audit, and Harness Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`18ed54a`. The previous backlog is implemented, validated, committed, and
pushed. The next mechanical-translation debt is concentrated in evidence rows
that still cite aggregate release-audit modules, remaining broad source audit
operation IDs, and release-audit modules that now block a lower module-size cap.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,899 unsafe
  mentions, and 413 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 413 operation IDs with
  881 automated evidence rows.
- No source audit operation ID remains over 32 source markers. Eight IDs remain
  over 28 markers: `arena-slice-boundary`, `owned-allocation`,
  `typed-subproc-accessors`, and `typed-scalar-test-fixtures` have 31 markers
  each; `arena-slice-allocation-boundary`, `page-free-list-refill-boundary`, and
  `stats-counter-boundary` have 30 markers each; and
  `raw-macro-cxx-aligned-boundary` has 29 markers.
- Issue 1 retargeted 184 dispatcher evidence rows. `docs/unsafe-audit-evidence.tsv`
  no longer cites the targeted aggregate or parent release-audit modules:
  root `release_fast_path_audit.rs`, `release_policy.rs`,
  `datatype_acceptance.rs`, `interposition_exports.rs`,
  `unsafe_performance.rs`, `allocation_free.rs`, `lifecycle_boundaries.rs`,
  `os_memory.rs`, `platform_primitive_boundaries.rs`, and
  `platform_primitive_boundaries/unix_wasi_emscripten.rs` all have zero
  evidence rows.
- Issue 2 split four broad source audit IDs into 26 operation-specific IDs:
  typed heap/thread-heap/TLD/address fixtures, platform default/cached
  thread-heap load/store and fallback paths, Windows virtual-memory operations,
  and subproc create/destroy/teardown operations.
- `strict-type-allowlist.txt` has 634 active entries: 332
  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40
  metadata-reference entries, 23 pointer-usize-roundtrip entries, 14 atomic-cast
  entries, and 5 heap-allocating-type entries. It has no retained wildcard
  entries. Strict-type wildcard guard: total=0; none.
- The largest strict-type allowlist file hotspots are `src/internal/mod.rs` with
  57 active entries, `src/allocator/alloc.rs` with 53,
  `src/allocator/page.rs` with 51, `src/allocator/aligned.rs` with 50,
  `src/internal/bitmap.rs` with 45, and `src/raw/macros.rs` with 32.
- The largest strict-type kind/file hotspots are `raw-pointer-parameter` in
  `src/allocator/alloc.rs` with 47 active entries, `raw-pointer-parameter` in
  `src/allocator/page.rs` with 44, `raw-pointer-parameter` in
  `src/internal/mod.rs` with 41, `bare-domain-usize` in
  `src/internal/bitmap.rs` with 33, `bare-domain-usize` in `src/raw/macros.rs`
  with 32, and `raw-pointer-parameter` in `src/allocator/aligned.rs` with 30.
- The largest raw-pointer domains are `mi_theap_t` with 111 entries,
  `mi_page_t` with 110, and `mi_heap_t` with 67.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  release-audit module-size guard discovers 107 module files recursively and
  enforces a 400-line cap. The largest release-audit modules are:
  `unsafe_performance/lifecycle_boundaries/thread_tls.rs` at 399 lines,
  `allowance_inventory.rs` at 398 lines,
  `interposition_exports/linux_override.rs` at 371 lines,
  `release_policy/security/unsafe_audit_evidence/focused_paths.rs` at 371 lines,
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch.rs` at 367 lines,
  and `primitive_options.rs` at 323 lines.
- Full validation for the previous backlog passed after Issue 6: `cargo fmt
  --all`, `cargo clippy --workspace --all-targets --all-features`, `cargo test
  --workspace --all-targets --all-features`, `cargo test --test
  release_fast_path_audit --all-features`, unsafe audit scripts, strict-type
  checks, no-allocation audit, unsafe inventory refresh/check, atomic ordering
  audit, and `git diff --check`.

## Issue 1: Repoint unsafe audit evidence to focused modules

The release-audit split made several parent modules into dispatchers, but the
evidence TSV still cites those parent paths for operation-specific evidence.
That weakens the evidence ownership model the previous backlog was trying to
make more precise.

- [x] Move evidence rows for split parent modules to the child module that owns
      the assertion: allocation/free marker groups, typed helper checks,
      OS/platform wrappers, Unix/WASI/Emscripten backends, and interposition
      export groups.
- [x] Split or retarget aggregate rows for `lifecycle_boundaries.rs`,
      `datatype_acceptance.rs`, `unsafe_performance.rs`,
      `platform_primitive_boundaries.rs`, and the root
      `release_fast_path_audit.rs` where a focused module already owns the
      check.
- [x] Tighten the release-audit evidence guard so parent dispatcher paths are
      accepted only for genuinely harness-wide policy checks, not
      operation-specific audit IDs.

Verify: unsafe audit evidence script, release fast-path audit, unsafe audit
script, full formatting, full clippy, and full tests.

## Issue 2: Split remaining source audit IDs over 32 markers

The source audit has one ID above the previous 32-marker target and three IDs
exactly at 32. These are still too broad for reviewable unsafe ownership.

- [x] Split `typed-heap-thread-tld-pointer-test-fixtures` into separate heap,
      thread-heap, TLD, and pointer-fixture IDs with matching evidence rows.
- [x] Split `platform-thread-heap-association-boundary` by default heap TLS
      load/store, cached heap TLS load/store, empty sentinel access, and
      platform fallback behavior.
- [x] Split `windows-virtual-memory-boundary` and
      `init-subproc-registry-boundary` by operation so each virtual-memory and
      subproc-registry concern has independent docs, source markers, and
      release-audit evidence.

Verify: unsafe audit scripts, unsafe evidence scripts, platform/init release
audit modules, full formatting, full clippy, and full tests.

## Issue 3: Lower the source audit marker cap to 28

After Issue 2, the next useful source-audit cap is 28 markers. The remaining
29-31 marker IDs are concentrated in arena slices, safe allocation ownership,
typed pointer accessors, scalar tests, page free-list refill, stats counters,
and C++ aligned raw overrides.

- [ ] Split `arena-slice-boundary`, `arena-slice-allocation-boundary`,
      `page-free-list-refill-boundary`, `stats-counter-boundary`, and
      `raw-macro-cxx-aligned-boundary` into narrower operation IDs.
- [ ] Split test-only broad IDs: `owned-allocation`,
      `typed-subproc-accessors`, and `typed-scalar-test-fixtures`.
- [ ] Update docs, evidence rows, and release-audit guards so no source audit
      operation ID exceeds 28 source markers.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 4: Split release-audit modules over 320 lines

The release-audit cap is now 400 lines. The next reviewability step is a
320-line cap, blocked by five modules.

- [ ] Split `allowance_inventory.rs` by allowance family so package lints,
      phase6 unsafe-op allowances, clippy allowances, dead-code allowances, and
      C compatibility allowances are independently reviewable.
- [ ] Split `thread_tls.rs`, `interposition_exports/linux_override.rs`, and
      `page_arena_behavior/memid_arena_dispatch.rs` by existing assertion
      groups without changing the release checks they perform.
- [ ] Split `primitive_options.rs` enough to lower the recursive release-audit
      module cap from 400 to 320 while preserving measured TODO checks.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Suggested implementation order

1. [x] Repoint unsafe audit evidence to focused modules.
2. [x] Split remaining source audit IDs over 32 markers.
3. [ ] Lower the source audit marker cap to 28.
4. [ ] Split release-audit modules over 320 lines.

## Definition of done for this backlog

- [x] Evidence rows for operation-specific audit IDs cite focused child modules
      instead of aggregate release-audit dispatchers.
- [ ] No source audit operation ID has more than 28 source markers.
- [ ] Release-audit modules are split enough to lower the recursive module cap
      to 320 lines without weakening discovery or measured TODO checks.
- [ ] Strict-type wildcard coverage remains at zero retained wildcards.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
