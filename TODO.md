# TODO: Next Audit Cluster, Strict-Type, and Harness Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`9e797a8`. The previous backlog is implemented, validated, committed, and
pushed. The next mechanical-translation debt is concentrated in broad source
audit operation IDs, strict-type allowlist hotspots, and release-audit modules
that now block a lower module-size cap.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,893 unsafe
  mentions, and 270 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 270 operation IDs with
  589 automated evidence rows.
- No source audit operation ID is over 60 source markers. The largest remaining
  source IDs are `os-memory-allocation-boundary` with 59 markers,
  `init-thread-heap-tls-boundary` and `arena-heap-page-visitors` with 56 markers
  each, `typed-atomic-domain` with 51 markers, `threadlocal-key-manager` and
  `ffi-libc-helpers` with 50 markers each, `page-queue-transition-boundary`
  with 49 markers, `init-thread-lifecycle-boundary` with 48 markers,
  `raw-macro-cxx-boundary` with 46 markers, and `page-map-init-boundary` with
  45 markers.
- `strict-type-allowlist.txt` has 634 active entries: 335
  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40
  metadata-reference entries, 23 pointer-usize-roundtrip entries, 14 atomic-cast
  entries, and 2 heap-allocating-type entries.
- The largest strict-type allowlist file hotspots are `src/internal/mod.rs` with
  57 active entries, `src/allocator/page.rs` with 54,
  `src/allocator/alloc.rs` with 53, `src/allocator/aligned.rs` with 50,
  `src/internal/bitmap.rs` with 45, and `src/raw/macros.rs` with 32. The largest
  raw-pointer domains are `mi_theap_t` with 114 entries, `mi_page_t` with 110,
  and `mi_heap_t` with 67.
- `strict-type-allowlist.txt` has 1 retained wildcard entry, the
  `heap-allocating-type` Loom proof-model boundary in
  `src/types/atomic/concurrency_models.rs`. Strict-type wildcard guard: total=1; heap-allocating-type=1.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  release-audit module cap is 600 lines; the only modules over 500 lines are
  `datatype_acceptance/strict_type.rs` at 592 lines,
  `release_policy/security.rs` at 584 lines,
  `release_policy/api_surface.rs` at 523 lines, and
  `release_policy/release_process.rs` at 504 lines.
- Full validation for Issue 5 passed: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets --all-features`,
  `cargo test --workspace --all-targets --all-features`,
  `cargo test --test release_fast_path_audit --all-features`, unsafe audit
  scripts, strict-type checks, no-allocation audit, unsafe inventory
  refresh/check, atomic ordering audit, and `git diff --check`.

## Issue 1: Split OS memory and page-map initialization audit clusters

> **Comment:** `os-memory-allocation-boundary` has 59 markers and still mixes
> reservation, commit/decommit, reset, free, remap, alignment, and memid work.
> `page-map-init-boundary` has 45 markers and still mixes global map bootstrap,
> submap allocation, initialization, and publication.

- [ ] Split `os-memory-allocation-boundary` by operation: reserve/commit,
      decommit/reset, aligned allocation, free/release, remap/protect, large-page
      handling, and memid/provenance helpers.
- [ ] Split `page-map-init-boundary` by operation: global map bootstrap, submap
      allocation/initialization, lazy publication, lookup base setup, and
      teardown safety.
- [ ] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, evidence rows, and
      release-audit guards so each new operation ID is named and counted
      independently.

Verify: OS memory tests, page-map tests, atomic ordering audit, unsafe audit
scripts, unsafe evidence scripts, full formatting, full clippy, and full tests.

## Issue 2: Split thread lifecycle and thread-local audit clusters

> **Comment:** `init-thread-heap-tls-boundary` has 56 markers,
> `threadlocal-key-manager` has 50 markers, and
> `init-thread-lifecycle-boundary` has 48 markers. These still group unrelated
> TLS storage, default-heap association, thread lifecycle, and cleanup paths.

- [ ] Split `init-thread-heap-tls-boundary` into focused IDs for TLS lookup,
      TLS publication, default heap association, main/static heap handling, and
      thread-local teardown.
- [ ] Split `init-thread-lifecycle-boundary` into focused IDs for thread init,
      thread done, TLD initialization, TLD cleanup, abandoned-page handling, and
      process/thread ordering checks.
- [ ] Split `threadlocal-key-manager` into focused IDs for key creation,
      key deletion, get/set operations, destructor callback handling, and
      platform TLS shim behavior.

Verify: lifecycle boundary release-audit modules, thread stats regression,
interposition export checks, strict-type checks, unsafe audit scripts, unsafe
evidence scripts, full formatting, full clippy, and full tests.

## Issue 3: Split arena visitor and page queue audit clusters

> **Comment:** `arena-heap-page-visitors` has 56 markers,
> `page-queue-transition-boundary` has 49 markers, and
> `internal-page-metadata-boundary` has 37 markers. They still hide several
> independent page ownership and visitation invariants behind broad IDs.

- [ ] Split `arena-heap-page-visitors` into focused IDs for block visitation,
      abandoned-block visitation, heap page iteration, page delete/destroy,
      page move/transfer, and visitor callback adapters.
- [ ] Split `page-queue-transition-boundary` into focused IDs for normal queue
      insertion/removal, full queue insertion/removal, abandoned queue
      transitions, delayed-free collection, and queue statistic updates.
- [ ] Split `internal-page-metadata-boundary` where release-audit guards already
      distinguish page size/count fields, free-list fields, heap/thread-heap
      ownership, and page flags.

Verify: page lifecycle and page queue tests, arena heap-page visitor
release-audit modules, strict-type checks, unsafe audit scripts, unsafe evidence
scripts, full formatting, full clippy, and full tests.

## Issue 4: Split atomic, FFI libc, and raw macro facade audit clusters

> **Comment:** `typed-atomic-domain` has 51 markers,
> `ffi-libc-helpers` has 50 markers, and `raw-macro-cxx-boundary` has 46
> markers. Each now covers multiple helper families that reviewers need to audit
> independently.

- [ ] Split `typed-atomic-domain` by wrapper family: usize/isize/i64 fields,
      pointer fields, page/thread-free atomics, arena-page atomics,
      guard/once/lock wrappers, and volatile/ordering helpers.
- [ ] Split `ffi-libc-helpers` by helper family: byte/string operations,
      fixed-buffer formatting, output callback routing, environment/path helpers,
      and no-allocation/no-unwind critical helpers.
- [ ] Split `raw-macro-cxx-boundary` by C++ facade family: scalar new/delete,
      array new/delete, nothrow variants, aligned variants, and heap-specific
      wrappers.

Verify: datatype acceptance strict-type modules, primitive option output tests,
raw facade release-audit modules, interposition exports, no-allocation audit,
unsafe audit scripts, unsafe evidence scripts, full formatting, full clippy, and
full tests.

## Issue 5: Tighten strict-type allowlist hotspots

> **Comment:** The strict-type allowlist is symbol-level, but its largest file
> hotspots still track broad translated-C domains: `src/internal/mod.rs` has 57
> active entries, `src/allocator/page.rs` has 54, `src/allocator/alloc.rs` has
> 53, and `src/allocator/aligned.rs` has 50.

- [ ] Add a release-audit guard that computes strict-type allowlist counts by
      kind, file, kind/file pair, and raw-pointer domain, then checks the current
      measured top entries against `TODO.md`.
- [ ] Audit the top raw-pointer hotspots and replace any compatibility entries
      that can be moved behind existing typed wrappers with narrower wrappers or
      remove the now-unneeded allowlist entries.
- [ ] Keep permanent allowlist entries symbol-level. Replace the final retained
      wildcard with exact proof-model coverage if the checker can model the Loom
      boundary without weakening hot-path heap-allocation detection; otherwise
      keep the wildcard count explicitly documented.

Verify: strict-type checker, datatype acceptance release-audit modules,
no-allocation audit, full formatting, full clippy, and full tests.

## Issue 6: Split release-audit modules over 500 lines and lower the cap

> **Comment:** Issue 5 lowered the cap to 600. The next release-audit bottleneck
> is now four files over 500 lines:
> `datatype_acceptance/strict_type.rs` at 592,
> `release_policy/security.rs` at 584,
> `release_policy/api_surface.rs` at 523, and
> `release_policy/release_process.rs` at 504.

- [ ] Split `datatype_acceptance/strict_type.rs` into focused modules for page
      thread/free bit domains, pointer/freelist wrappers, atomic/provenance
      guards, metadata-reference guards, strict-checker self-tests, and
      allowlist inventory checks.
- [ ] Split `release_policy/security.rs` into focused modules for advisory/CVE
      policy, fuzz/reproducer policy, unsafe-audit evidence ownership,
      hardening threat model, and memory-corruption triage.
- [ ] Split `release_policy/api_surface.rs` and
      `release_policy/release_process.rs` into topic modules matching their
      existing tests, then lower the release-audit module cap from 600 to 500
      without weakening coverage.

Verify: release fast-path audit, docs/security/release policy modules,
strict-type checks, unsafe audit scripts, unsafe evidence scripts, full
formatting, full clippy, and full tests.

## Suggested implementation order

1. [ ] Split OS memory and page-map initialization audit clusters.
2. [ ] Split thread lifecycle and thread-local audit clusters.
3. [ ] Split arena visitor and page queue audit clusters.
4. [ ] Split atomic, FFI libc, and raw macro facade audit clusters.
5. [ ] Tighten strict-type allowlist hotspots.
6. [ ] Split release-audit modules over 500 lines and lower the cap.

## Definition of done for this backlog

- [ ] No source audit operation ID has more than 40 source markers.
- [ ] The largest source audit clusters listed above are split into narrower
      review units with separate docs, evidence rows, and release-audit guards.
- [ ] Strict-type allowlist hotspots are measured in release-audit coverage, and
      any removed or retained entries have explicit symbol-level rationale.
- [ ] Release-audit modules are split enough to lower the module cap to 500
      lines without weakening coverage.
- [ ] The strict-type wildcard guard either reaches zero retained wildcards or
      remains explicitly documented with current measured evidence.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
