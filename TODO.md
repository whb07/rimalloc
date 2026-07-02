# TODO: Next Audit Cluster, Strict-Type, and Harness Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`9e797a8`. The previous backlog is implemented, validated, committed, and
pushed. The next mechanical-translation debt is concentrated in broad source
audit operation IDs, strict-type allowlist hotspots, and release-audit modules
that now block a lower module-size cap.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,893 unsafe
  mentions, and 337 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 337 operation IDs with
  725 automated evidence rows.
- No source audit operation ID is over 39 source markers. The largest remaining
  source IDs are `options-message-output-boundary` with 39 markers,
  `arena-purge-boundary` with 38 markers, `arena-registry-boundary` and
  `allocation-realloc-entry-boundary` with 37 markers each,
  `typed-page-state-domain`, `raw-macro-override-entry-boundary`, and
  `init-subproc-heap-association-boundary` with 36 markers each,
  `typed-heap-thread-tld-pointer-test-fixtures` with 35 markers, and
  `arena-debug-render` and `allocation-heap-dispatch-boundary` with 34 markers
  each.
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
  `release_policy/security.rs` at 600 lines,
  `datatype_acceptance/strict_type.rs` at 598 lines,
  `release_policy/api_surface.rs` at 523 lines, and
  `release_policy/release_process.rs` at 504 lines.
- Full validation for Issue 4 passed: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets --all-features`,
  `cargo test --workspace --all-targets --all-features`,
  `cargo test --test release_fast_path_audit --all-features`, unsafe audit
  scripts, strict-type checks, no-allocation audit, unsafe inventory
  refresh/check, atomic ordering audit, and `git diff --check`.
- Issue 1 split `os-memory-allocation-boundary` into primitive allocation,
  aligned allocation, memid/provenance, zeroing, direct request allocation, and
  public export IDs. It split `page-map-init-boundary` into 32-bit commit
  bitmap, 32-bit bootstrap, top-level lazy commit, global bootstrap, initial
  submap, and null-sentinel IDs.
- Issue 2 split `init-thread-heap-tls-boundary` into empty sentinel, main TLS,
  main static metadata, default heap association, cached heap association, and
  safe-access IDs. It split `init-thread-lifecycle-boundary` into thread
  identity, thread-done entry, thread-done dispatch, default-heap init,
  thread-done cleanup, and thread-init entry IDs. It split
  `threadlocal-key-manager` into empty sentinel, TLS shim, slot block, claim
  callback, key claim/free/create/access, free bitmap, global manager, and
  thread-done IDs.
- Issue 3 split `arena-heap-page-visitors` into visit callback, block visit,
  page iteration, abandoned visit, delete, transfer, delete dispatch, move, and
  destroy IDs. It split `page-queue-transition-boundary` into full transition,
  free release, retire, abandon, reclaim, validation, direct-cache, link
  mutation, and append IDs. It split `internal-page-metadata-boundary` into
  direct-bin, page-map lookup, size/count, owner/heap, location, and flag/owner
  IDs while retaining free-list link helpers under `internal-block-link-boundary`.
- Issue 4 split `typed-atomic-domain` into usize field, i64 field, pointer
  field, page thread-id, thread-free, abandoned count, thread-heap refcount,
  arena-page, and guard/once/lock IDs. It split `ffi-libc-helpers` into string,
  environment, output, format, and retention IDs. It split
  `raw-macro-cxx-boundary` into delete, array delete, aligned, nothrow, and new
  IDs.

## Issue 1: Split OS memory and page-map initialization audit clusters

> **Comment:** Completed. The stale broad source IDs
> `os-memory-allocation-boundary` and `page-map-init-boundary` are gone from
> allocator source and are now explicitly rejected by release-audit guards.

- [x] Split `os-memory-allocation-boundary` by operation: reserve/commit,
      decommit/reset, aligned allocation, free/release, remap/protect, large-page
      handling, and memid/provenance helpers.
- [x] Split `page-map-init-boundary` by operation: global map bootstrap, submap
      allocation/initialization, lazy publication, lookup base setup, and
      teardown safety.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, evidence rows, and
      release-audit guards so each new operation ID is named and counted
      independently.

Verify: OS memory tests, page-map tests, atomic ordering audit, unsafe audit
scripts, unsafe evidence scripts, full formatting, full clippy, and full tests.

## Issue 2: Split thread lifecycle and thread-local audit clusters

> **Comment:** Completed. The stale broad source IDs
> `init-thread-heap-tls-boundary`, `init-thread-lifecycle-boundary`, and
> `threadlocal-key-manager` are gone from allocator source and are now
> explicitly rejected by release-audit guards.

- [x] Split `init-thread-heap-tls-boundary` into focused IDs for TLS lookup,
      TLS publication, default heap association, main/static heap handling, and
      thread-local teardown.
- [x] Split `init-thread-lifecycle-boundary` into focused IDs for thread init,
      thread done, TLD initialization, TLD cleanup, abandoned-page handling, and
      process/thread ordering checks.
- [x] Split `threadlocal-key-manager` into focused IDs for key creation,
      key deletion, get/set operations, destructor callback handling, and
      platform TLS shim behavior.

Verify: lifecycle boundary release-audit modules, thread stats regression,
interposition export checks, strict-type checks, unsafe audit scripts, unsafe
evidence scripts, full formatting, full clippy, and full tests.

## Issue 3: Split arena visitor and page queue audit clusters

> **Comment:** Completed. The stale broad source IDs
> `arena-heap-page-visitors`, `page-queue-transition-boundary`, and
> `internal-page-metadata-boundary` are gone from allocator source and are now
> explicitly rejected by release-audit guards.

- [x] Split `arena-heap-page-visitors` into focused IDs for block visitation,
      abandoned-block visitation, heap page iteration, page delete/destroy,
      page move/transfer, and visitor callback adapters.
- [x] Split `page-queue-transition-boundary` into focused IDs for normal queue
      insertion/removal, full queue insertion/removal, abandoned queue
      transitions, delayed-free collection, and queue statistic updates.
- [x] Split `internal-page-metadata-boundary` where release-audit guards already
      distinguish page size/count fields, free-list fields, heap/thread-heap
      ownership, and page flags.

Verify: page lifecycle and page queue tests, arena heap-page visitor
release-audit modules, strict-type checks, unsafe audit scripts, unsafe evidence
scripts, full formatting, full clippy, and full tests.

## Issue 4: Split atomic, FFI libc, and raw macro facade audit clusters

> **Comment:** Completed. The stale broad source IDs `typed-atomic-domain`,
> `ffi-libc-helpers`, and `raw-macro-cxx-boundary` are gone from allocator
> source and are now explicitly rejected by release-audit guards.

- [x] Split `typed-atomic-domain` by wrapper family: usize/isize/i64 fields,
      pointer fields, page/thread-free atomics, arena-page atomics,
      guard/once/lock wrappers, and volatile/ordering helpers.
- [x] Split `ffi-libc-helpers` by helper family: byte/string operations,
      fixed-buffer formatting, output callback routing, environment/path helpers,
      and no-allocation/no-unwind critical helpers.
- [x] Split `raw-macro-cxx-boundary` by C++ facade family: scalar new/delete,
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
> `release_policy/security.rs` at 600,
> `datatype_acceptance/strict_type.rs` at 598,
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

1. [x] Split OS memory and page-map initialization audit clusters.
2. [x] Split thread lifecycle and thread-local audit clusters.
3. [x] Split arena visitor and page queue audit clusters.
4. [x] Split atomic, FFI libc, and raw macro facade audit clusters.
5. [ ] Tighten strict-type allowlist hotspots.
6. [ ] Split release-audit modules over 500 lines and lower the cap.

## Definition of done for this backlog

- [x] No source audit operation ID has more than 40 source markers.
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
