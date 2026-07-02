# TODO: Fresh-Eyes Audit Density, Module Cap, and Strict-Type Classification Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`5c47626`. That backlog is implemented and pushed: source audit marker cap 22,
release-audit module cap 260, and strict-type permanent reason/file-domain
hotspot governance. Issue 1 in this backlog has now lowered the source audit
marker cap to 20. The remaining mechanical-translation debt is concentrated in
release-audit modules just above a 240-line review target and broad strict-type permanent reason buckets
that still hide different compatibility responsibilities.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,899 unsafe
  mentions, and 504 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 504 operation IDs with
  1,076 automated evidence rows.
- No source audit operation ID has more than 20 source markers. Six IDs now sit
  at the 20-marker cap: `stats-json-boundary`,
  `raw-macro-unix-aligned-posix-boundary`, `raw-macro-linux-libc-boundary`,
  `internal-atomic-platform-lock-boundary`, `emscripten-memory-boundary`, and
  `allocation-string-path-boundary`.
- `strict-type-allowlist.txt` has 634 active entries: 332
  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40
  metadata-reference entries, 23 pointer-usize-roundtrip entries, 14 atomic-cast
  entries, and 5 heap-allocating-type entries. It has zero wildcard entries and
  zero removable entries.
- Compatibility snapshot: It has no retained wildcard
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
- The largest strict-type permanent reason groups are allocation compatibility
  boundary with 47 entries, typed-domain constructor boundary with 42 entries,
  internal translated C helper boundary with 41 entries, C ABI parity or
  translated-C source shape with 33 entries, C/platform ABI forwarding boundary
  with 32 entries, and aligned allocation compatibility boundary with 30
  entries. The next useful permanent reason cap is 40 entries.
- The largest strict-type raw-pointer file/domain hotspots are
  `src/internal/mod.rs`/`mi_page_t` with 33 entries,
  `src/allocator/alloc.rs`/`mi_theap_t` with 30,
  `src/allocator/page.rs`/`mi_page_t` with 25,
  `src/allocator/thread_heap.rs`/`mi_theap_t` with 20,
  `src/allocator/free.rs`/`mi_page_t` with 19, and
  `src/allocator/page.rs`/`mi_theap_t` with 19. The current raw-pointer
  file/domain cap is 33 entries.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  release-audit module-size guard discovers 135 module files recursively and
  enforces a 260-line cap. The next useful cap is 240 lines. The largest
  release-audit modules are:
  `strict_policy.rs` at 260 lines, `release_policy/callbacks.rs` at 255 lines,
  `datatype_acceptance/domain_wrappers.rs` at 253 lines,
  `unsafe_performance/allocation_free/marker_counts.rs` at 252 lines,
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs` at 248 lines, and
  `release_policy/security/unsafe_audit_evidence/focused_paths.rs` at 247 lines.
- Ten release-audit modules currently exceed 240 lines: `strict_policy.rs`,
  `release_policy/callbacks.rs`, `datatype_acceptance/domain_wrappers.rs`,
  `unsafe_performance/allocation_free/marker_counts.rs`,
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs`,
  `release_policy/security/unsafe_audit_evidence/focused_paths.rs`,
  `unsafe_performance/page_lifecycle.rs`,
  `unsafe_performance/platform_primitive_boundaries/macos.rs`,
  `unsafe_performance/page_arena_behavior/arena_page_allocation.rs`, and
  `unsafe_performance/bitmap_boundaries.rs`.

## Issue 1: Lower the source audit marker cap to 20

Done. The source audit cap is now enforced by a release-audit guard that scans
all `src/**/*.rs` `SAFETY[AUDIT:*]` IDs and fails if any operation ID exceeds
20 source markers.

- [x] Split the former 22-marker IDs by responsibility:
      `macos-zone-allocation-boundary`, `windows-large-page-boundary`,
      `arena-abandoned-boundary`, `arena-page-fresh-boundary`,
      `internal-atomic-fetch-boundary`, `page-count-accessors`,
      `typed-callback-test-fixtures`, and `typed-page-queue-test-fixtures`.
- [x] Split the former 21-marker IDs by operation family:
      `os-memory-public-export-boundary`, `init-thread-done-cleanup-boundary`,
      `arena-slice-allocation-bitmaps-boundary`, and
      `allocation-product-entry-boundary`.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, evidence rows, and
      release-audit guards so no source audit operation ID exceeds 20 source
      markers.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 2: Split release-audit modules over 240 lines

The recursive release-audit module cap is now 260 lines. Ten modules exceed the
next 240-line review target.

- [ ] Split `strict_policy.rs` so the strict-type hot-path, no-allocation, and
      datatype acceptance checks live in focused child modules while the root
      harness remains small.
- [ ] Split `release_policy/callbacks.rs`,
      `datatype_acceptance/domain_wrappers.rs`, and
      `unsafe_performance/allocation_free/marker_counts.rs` by assertion
      family.
- [ ] Split the remaining 241-248 line modules:
      `unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs`,
      `release_policy/security/unsafe_audit_evidence/focused_paths.rs`,
      `unsafe_performance/page_lifecycle.rs`,
      `unsafe_performance/platform_primitive_boundaries/macos.rs`,
      `unsafe_performance/page_arena_behavior/arena_page_allocation.rs`, and
      `unsafe_performance/bitmap_boundaries.rs`.
- [ ] Lower the recursive release-audit module cap from 260 to 240 and refresh
      measured TODO evidence.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Issue 3: Split strict-type permanent reason groups over 40

The strict-type allowlist now guards permanent reason groups, but three
permanent disposition buckets still exceed the next 40-entry cap.

- [ ] Split `allocation compatibility boundary` into more specific permanent
      disposition families for thread-heap allocation, heap export allocation,
      realloc/product helpers, and C++/guarded allocation edges.
- [ ] Split `typed-domain constructor boundary` into scalar, arena/memid, and
      test/proof constructor groups where the current broad reason hides
      different invariants.
- [ ] Split `internal translated C helper boundary` into page metadata,
      thread-heap, and free-list helper families while keeping the raw-pointer
      file/domain cap at 33.
- [ ] Refresh `docs/strict-types.md`, `strict-type-allowlist.txt`, release-audit
      guards, and TODO evidence so no permanent reason group exceeds 40 entries.

Verify: release fast-path audit, strict-type checker, full formatting, full
clippy, and full tests.

## Suggested implementation order

1. [x] Lower the source audit marker cap to 20.
2. [ ] Split release-audit modules over 240 lines.
3. [ ] Split strict-type permanent reason groups over 40.

## Definition of done for this backlog

- [x] No source audit operation ID has more than 20 source markers.
- [ ] Release-audit modules are split enough to lower the recursive module cap
      to 240 lines without weakening discovery or measured TODO checks.
- [ ] Strict-type permanent disposition reason groups are split enough that no
      group exceeds 40 entries, while raw-pointer file/domain hotspot governance
      remains at or below the 33-entry cap.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
