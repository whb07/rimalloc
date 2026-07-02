# TODO: Fresh-Eyes Audit Density, Module Cap, and Strict-Type Classification Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`5c47626`. That backlog is implemented and pushed: source audit marker cap 22,
release-audit module cap 260, and strict-type permanent reason/file-domain
hotspot governance. Issue 1 in this backlog has now lowered the source audit
marker cap to 20, Issue 2 has split the release-audit modules enough to enforce
a 240-line recursive cap, and Issue 3 has split the strict-type permanent
reason groups enough to enforce a 40-entry cap.

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
- No strict-type permanent reason group exceeds 40 entries. The largest groups
  are internal page metadata helper boundary with 33 entries, C ABI parity or
  translated-C source shape with 33 entries, C/platform ABI forwarding boundary
  with 32 entries, aligned allocation compatibility boundary with 30 entries,
  thread-heap compatibility boundary with 24 entries, and scalar domain
  constructor boundary with 24 entries.
- The largest strict-type raw-pointer file/domain hotspots are
  `src/internal/mod.rs`/`mi_page_t` with 33 entries,
  `src/allocator/alloc.rs`/`mi_theap_t` with 30,
  `src/allocator/page.rs`/`mi_page_t` with 25,
  `src/allocator/thread_heap.rs`/`mi_theap_t` with 20,
  `src/allocator/free.rs`/`mi_page_t` with 19, and
  `src/allocator/page.rs`/`mi_theap_t` with 19. The current raw-pointer
  file/domain cap is 33 entries.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  release-audit module-size guard discovers 145 module files recursively and
  enforces a 240-line cap. The largest release-audit modules are:
  `unsafe_performance/os_memory/platform_helpers.rs` at 240 lines,
  `unsafe_performance/platform_primitive_boundaries/unix_wasi_emscripten/emscripten.rs` at 237 lines,
  `unsafe_performance/platform_primitive_boundaries/windows.rs` at 233 lines,
  `unsafe_performance/page_arena_behavior/purge_clock_atomic.rs` at 232 lines,
  `datatype_acceptance/strict_type/atomic_provenance_guards.rs` at 231 lines,
  and
  `unsafe_performance/lifecycle_boundaries/process_lifecycle/subproc_fast_paths.rs` at 223 lines.
- No release-audit module currently exceeds 240 lines.

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

Done. The recursive release-audit module cap is now 240 lines.

- [x] Split `strict_policy.rs` so the strict-type hot-path, no-allocation, and
      datatype acceptance checks live in focused child modules while the root
      harness remains small.
- [x] Split `release_policy/callbacks.rs`,
      `datatype_acceptance/domain_wrappers.rs`, and
      `unsafe_performance/allocation_free/marker_counts.rs` by assertion
      family.
- [x] Split the remaining 241-248 line modules:
      `unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs`,
      `release_policy/security/unsafe_audit_evidence/focused_paths.rs`,
      `unsafe_performance/page_lifecycle.rs`,
      `unsafe_performance/platform_primitive_boundaries/macos.rs`,
      `unsafe_performance/page_arena_behavior/arena_page_allocation.rs`, and
      `unsafe_performance/bitmap_boundaries.rs`.
- [x] Lower the recursive release-audit module cap from 260 to 240 and refresh
      measured TODO evidence.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Issue 3: Split strict-type permanent reason groups over 40

Done. The strict-type allowlist now keeps every permanent reason group at or
below the 40-entry cap.

- [x] Split `allocation compatibility boundary` into more specific permanent
      disposition families for thread-heap allocation, heap export allocation,
      realloc/product helpers, and C++/guarded allocation edges.
- [x] Split `typed-domain constructor boundary` into scalar, arena/memid, and
      test/proof constructor groups where the current broad reason hides
      different invariants.
- [x] Split `internal translated C helper boundary` into page metadata,
      thread-heap, and free-list helper families while keeping the raw-pointer
      file/domain cap at 33.
- [x] Refresh `docs/strict-types.md`, `strict-type-allowlist.txt`, release-audit
      guards, and TODO evidence so no permanent reason group exceeds 40 entries.

Verify: release fast-path audit, strict-type checker, full formatting, full
clippy, and full tests.

## Suggested implementation order

1. [x] Lower the source audit marker cap to 20.
2. [x] Split release-audit modules over 240 lines.
3. [x] Split strict-type permanent reason groups over 40.

## Definition of done for this backlog

- [x] No source audit operation ID has more than 20 source markers.
- [x] Release-audit modules are split enough to lower the recursive module cap
      to 240 lines without weakening discovery or measured TODO checks.
- [x] Strict-type permanent disposition reason groups are split enough that no
      group exceeds 40 entries, while raw-pointer file/domain hotspot governance
      remains at or below the 33-entry cap.
- [x] `TODO.md` and its release-audit guard reflect the current measured counts.
- [x] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
