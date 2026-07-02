# TODO: Fresh-Eyes Audit Cap, Harness, and Strict-Type Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`8ade753` and lowering the source audit marker cap to 24. The previous backlog
and Issue 1 below are implemented. The remaining mechanical-translation debt is
concentrated in release-audit modules still clustered around 300 lines and
strict-type allowlist hotspots that need sharper governance before deeper typed
rewrites.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,899 unsafe
  mentions, and 462 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 462 operation IDs with
  990 automated evidence rows.
- No source audit operation ID has more than 24 source markers. The IDs at the
  current cap are `os-memory-config-boundary`, `typed-tld-accessors`, and
  `arena-id-memid-helpers` at 24 markers each.
- The retired broad marker families have been split into focused source IDs for
  thread-heap visitation, stats output, free validation, Windows TLS/FLS, typed
  page/arena accessors, libc string helpers, unsafe callback guards, Unix memory
  primitives, and heap aligned realloc variants.
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
  release-audit module-size guard discovers 119 module files recursively and
  enforces a 320-line cap. The next useful cap is 280 lines. Five modules still
  exceed that target. The largest release-audit modules are:
  `unsafe_performance/lifecycle_boundaries/thread_tls.rs` at 314 lines,
  `unsafe_performance/platform_primitive_boundaries/windows.rs` at 306 lines,
  `release_policy/callbacks.rs` at 304 lines,
  `release_policy/api_surface/raw_facade.rs` at 302 lines, and
  `unsafe_performance/thread_stats.rs` at 284 lines, and
  `interposition_exports/linux_override.rs` at 279 lines.
- Review sampling found the largest release-audit modules still mix independent
  assertion families: thread/TLS lifecycle, callback ABI versus safe guard policy,
  raw facade versus nullability policy, Windows TLS/FLS versus virtual memory policy,
  stats output versus marker inventory, and Linux override export policy.
- Validation for commit `8ade753` passed: `cargo fmt --all`, `cargo clippy
  --workspace --all-targets --all-features`, `cargo test --workspace
  --all-targets --all-features`, `cargo test --test release_fast_path_audit
  --all-features`, unsafe audit scripts, strict-type checks, no-allocation
  audit, unsafe inventory refresh/check, atomic ordering audit, and
  `git diff --check`.

## Issue 1: Lower the source audit marker cap to 24

Completed in this pass. The source audit cap is now measured at 24 markers and
the missed 25-marker `typed-page-accessors` hotspot was split as part of the
same cap-lowering work.

- [x] Split `thread-heap-visit-boundary` into independent visitation,
      callback-argument, page-area, and thread-heap helper audit IDs.
- [x] Split `stats-output-boundary` by stack-buffer formatting, output callback
      forwarding, print/export C ABI paths, and stats-output test fixtures.
- [x] Split `free-validation-boundary` by null/alignment validation,
      pointer-page lookup, interior/guarded lookup, double-free checks, and
      local/remote/abandoned validation fixtures.
- [x] Split the 25-27 marker IDs by operation family:
      `windows-tls-fls-boundary`, `typed-arena-accessors`,
      `typed-page-accessors`, `ffi-libc-string-boundary`, `unsafe-callback`,
      `unix-mmap-syscall-boundary`, and
      `aligned-realloc-heap-export-boundary`.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, evidence rows, and
      release-audit guards so no source audit operation ID exceeds 24 source
      markers.

Verify: unsafe audit scripts, unsafe evidence scripts, release fast-path audit,
full formatting, full clippy, and full tests.

## Issue 2: Split release-audit modules over 280 lines

The release-audit cap is now 320 lines. Five modules still exceed the next
280-line target and mix assertion families that should be independently
reviewable.

- [ ] Split `unsafe_performance/lifecycle_boundaries/thread_tls.rs` by thread
      lifecycle, TLS/default-heap checks, visitation checks, and reclaim helpers.
- [x] Split `release_policy/security/unsafe_audit_evidence/focused_paths.rs`
      below 280 lines by moving platform primitive evidence checks into a
      focused child module.
- [ ] Split `release_policy/callbacks.rs` into callback ABI inventory,
      no-alloc/no-unwind trampoline policy, and safe callback guard policy.
- [ ] Split `release_policy/api_surface/raw_facade.rs` into raw facade shape,
      raw macro policy, repository import policy, and raw pointer nullability
      checks.
- [ ] Split `unsafe_performance/platform_primitive_boundaries/windows.rs` and
      `unsafe_performance/thread_stats.rs` enough to lower the recursive module
      cap from 320 to 280 while preserving measured TODO checks.

Verify: release fast-path audit, TODO guard, unsafe audit scripts, strict-type
checks, no-allocation audit, full formatting, full clippy, and full tests.

## Issue 3: Tighten strict-type allowlist hotspot governance

The allowlist has no wildcards or removable entries, but the current inventory
still reads as a flat mechanical list. The next step is to make the permanent
boundary categories more reviewable before attempting deeper typed rewrites.

- [ ] Split the strict-type allowlist inventory release-audit checks by concern:
      active entry counts, kind/file hotspots, raw-pointer domains, and
      wildcard/removable-entry policy.
- [ ] Add measured guards for the top raw-pointer domains and strict-type
      hotspot files so future work cannot silently grow the `mi_theap_t`,
      `mi_page_t`, `mi_heap_t`, `src/internal/mod.rs`,
      `src/allocator/alloc.rs`, or `src/allocator/page.rs` debt.
- [ ] Document which hotspot families are permanent C ABI compatibility edges
      and which are candidates for future typed helper rewrites.

Verify: release fast-path audit, strict-type checker, full formatting, full
clippy, and full tests.

## Suggested implementation order

1. [x] Lower the source audit marker cap to 24.
2. [ ] Split release-audit modules over 280 lines.
3. [ ] Tighten strict-type allowlist hotspot governance.

## Definition of done for this backlog

- [x] No source audit operation ID has more than 24 source markers.
- [ ] Release-audit modules are split enough to lower the recursive module cap
      to 280 lines without weakening discovery or measured TODO checks.
- [ ] Strict-type allowlist hotspot governance is split, measured, and tied to
      permanent-versus-rewriteable boundary documentation.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
