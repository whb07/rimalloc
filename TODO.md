# TODO: Fresh-Eyes Audit Cap, Harness, and Strict-Type Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`7af25fa`. The previous backlog is implemented and pushed: source audit marker
cap 24, release-audit module cap 280, and strict-type allowlist hotspot
governance. The next mechanical-translation debt is concentrated in audit IDs
now sitting at the 22-marker source cap, release-audit modules at the tightened
harness cap, and strict-type allowlist reason/domain hotspots. Issues 1, 2, and
3 of this backlog have been implemented and validated locally; the final
fresh-eyes review remains.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,899 unsafe
  mentions, and 492 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 492 operation IDs with
  1,052 automated evidence rows.
- No source audit operation ID has more than 22 source markers. The old
  over-cap IDs are split into responsibility-level marker families in source,
  `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, evidence rows, and release-audit
  guards.
- Eight IDs now sit at the 22-marker cap: `macos-zone-allocation-boundary`,
  `windows-large-page-boundary`, `arena-abandoned-boundary`,
  `arena-page-fresh-boundary`, `internal-atomic-fetch-boundary`,
  `page-count-accessors`, `typed-callback-test-fixtures`, and
  `typed-page-queue-test-fixtures`.
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
  entries.
- The largest strict-type raw-pointer file/domain hotspots are
  `src/internal/mod.rs`/`mi_page_t` with 33 entries,
  `src/allocator/alloc.rs`/`mi_theap_t` with 30,
  `src/allocator/page.rs`/`mi_page_t` with 25,
  `src/allocator/thread_heap.rs`/`mi_theap_t` with 20,
  `src/allocator/free.rs`/`mi_page_t` with 19, and
  `src/allocator/page.rs`/`mi_theap_t` with 19.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  release-audit module-size guard discovers 134 module files recursively and
  enforces a 260-line cap. The largest release-audit modules are:
  `strict_policy.rs` at 260 lines, `release_policy/callbacks.rs` at 255 lines,
  `datatype_acceptance/domain_wrappers.rs` at 253 lines,
  `unsafe_performance/allocation_free/marker_counts.rs` at 251 lines,
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch/memid_view.rs` at 248 lines, and
  `release_policy/security/unsafe_audit_evidence/focused_paths.rs` at 247 lines.

## Issue 1: Lower the source audit marker cap to 22

Completed in this pass. The current 22-marker cap is enforced, and the former
23/24-marker operation IDs now have focused responsibility-level marker
families.

- [x] Split the 24-marker IDs by responsibility:
      `os-memory-config-boundary`, `typed-tld-accessors`, and
      `arena-id-memid-helpers`.
- [x] Split the 23-marker IDs by operation family:
      `aligned-padding-boundary`, `arena-page-free-boundary`,
      `arena-page-reabandon-unabandon-boundary`, `heap-ownership-query-boundary`,
      `stats-counter-update-boundary`, `typed-freelist-wrappers`, and
      `scalar-domain-wrappers`.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, evidence rows, and
      release-audit guards so no source audit operation ID exceeds 22 source
      markers.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 2: Split release-audit modules over 260 lines

Completed in this pass. The release-audit cap is now 260 lines, and
`strict_policy.rs` sits exactly at that target. The former 279-line audit
modules are now split into focused assertion families.

- [x] Split `interposition_exports/linux_override.rs` into Linux TLS shim,
      override export, and C++/static-link compatibility assertion families.
- [x] Split `primitive_options.rs` into option storage, output/formatting, and
      libc string helper assertion families.
- [x] Keep `strict_policy.rs` at or below 260 lines while preserving the new
      strict-type allowlist governance child module.
- [x] Lower the recursive release-audit module cap from 280 to 260 and refresh
      measured TODO evidence.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Issue 3: Guard strict-type permanent reason and file/domain hotspots

Completed in this pass. The strict-type allowlist now has active-entry,
kind/file, domain, wildcard, removable-entry, permanent reason-group, and
raw-pointer file/domain hotspot governance.

- [x] Add release-audit guards for the top permanent disposition reason groups:
      allocation compatibility boundary, typed-domain constructor boundary,
      internal translated C helper boundary, C ABI parity/source-shape
      boundary, C/platform ABI forwarding boundary, and aligned allocation
      compatibility boundary.
- [x] Add measured guards for the top raw-pointer file/domain pairs so the
      `src/internal/mod.rs`/`mi_page_t`, `src/allocator/alloc.rs`/`mi_theap_t`,
      `src/allocator/page.rs`/`mi_page_t`, `src/allocator/thread_heap.rs`/
      `mi_theap_t`, `src/allocator/free.rs`/`mi_page_t`, and
      `src/allocator/page.rs`/`mi_theap_t` hotspots cannot silently grow.
- [x] Update `docs/strict-types.md` to classify those reason groups as
      permanent ABI/source-shape edges or future typed-helper rewrite
      candidates.

Verify: release fast-path audit, strict-type checker, full formatting, full
clippy, and full tests.

## Suggested implementation order

1. [x] Lower the source audit marker cap to 22.
2. [x] Split release-audit modules over 260 lines.
3. [x] Guard strict-type permanent reason and file/domain hotspots.

## Definition of done for this backlog

- [x] No source audit operation ID has more than 22 source markers.
- [x] Release-audit modules are split enough to lower the recursive module cap
      to 260 lines without weakening discovery or measured TODO checks.
- [x] Strict-type permanent reason and file/domain hotspot governance is
      measured, documented, and tied to permanent-versus-rewriteable boundary
      classifications.
- [x] `TODO.md` and its release-audit guard reflect the current measured counts.
- [x] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
