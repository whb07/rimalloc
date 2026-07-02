# TODO: Next Audit Cluster, Strict-Type, and Harness Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`f34084c`. The previous backlog is implemented, validated, committed, and
pushed. The next mechanical-translation debt is concentrated in the remaining
broad source audit operation IDs, strict-type allowlist hotspots, and
release-audit modules that now block a lower module-size cap.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,899 unsafe
  mentions, and 391 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 391 operation IDs with
  842 automated evidence rows.
- No source audit operation ID is over 35 source markers. The largest remaining
  source ID is `typed-heap-thread-tld-pointer-test-fixtures` with 35 markers.
  `init-subproc-registry-boundary`, `platform-thread-heap-association-boundary`,
  and `windows-virtual-memory-boundary` have 32 markers each.
- `strict-type-allowlist.txt` has 634 active entries: 332
  raw-pointer-parameter entries, 220 bare-domain-usize entries, 40
  metadata-reference entries, 23 pointer-usize-roundtrip entries, 14 atomic-cast
  entries, and 5 heap-allocating-type entries.
- The largest strict-type allowlist file hotspots are `src/internal/mod.rs` with
  57 active entries, `src/allocator/alloc.rs` with 53,
  `src/allocator/page.rs` with 51, `src/allocator/aligned.rs` with 50,
  `src/internal/bitmap.rs` with 45, and `src/raw/macros.rs` with 32. The largest
  raw-pointer domains are `mi_theap_t` with 111 entries, `mi_page_t` with 110,
  and `mi_heap_t` with 67.
- The largest strict-type kind/file hotspots are `raw-pointer-parameter` in
  `src/allocator/alloc.rs` with 47 active entries, `raw-pointer-parameter` in
  `src/allocator/page.rs` with 44, `raw-pointer-parameter` in
  `src/internal/mod.rs` with 41, `bare-domain-usize` in
  `src/internal/bitmap.rs` with 33, `bare-domain-usize` in `src/raw/macros.rs`
  with 32, and `raw-pointer-parameter` in `src/allocator/aligned.rs` with 30.
- `strict-type-allowlist.txt` has no retained wildcard entries. The Loom
  proof-model heap-allocation boundary in
  `src/types/atomic/concurrency_models.rs` is covered by exact test-function
  symbols. Strict-type wildcard guard: total=0; none.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  release-audit module-size guard discovers 107 module files recursively and
  enforces a 400-line cap. The largest release-audit modules are:
  `allowance_inventory.rs` at 398 lines,
  `unsafe_performance/lifecycle_boundaries/thread_tls.rs` at 390 lines,
  `interposition_exports/linux_override.rs` at 371 lines,
  `unsafe_performance/page_arena_behavior/memid_arena_dispatch.rs` at 367 lines,
  `primitive_options.rs` at 323 lines, and
  `unsafe_performance/thread_stats.rs` at 319 lines.
- Full validation for Issue 4 passed: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets --all-features`,
  `cargo test --workspace --all-targets --all-features`,
  `cargo test --test release_fast_path_audit --all-features`, unsafe audit
  scripts, strict-type checks, no-allocation audit, unsafe inventory
  refresh/check, atomic ordering audit, and `git diff --check`.
- Full validation for Issue 5 passed: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets --all-features`,
  `cargo test --workspace --all-targets --all-features`,
  `cargo test --test release_fast_path_audit --all-features`, unsafe audit
  scripts, strict-type checks, no-allocation audit, unsafe inventory
  refresh/check, atomic ordering audit, and `git diff --check`.
- Full validation for Issue 6 passed: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets --all-features`,
  `cargo test --workspace --all-targets --all-features`,
  `cargo test --test release_fast_path_audit --all-features`, unsafe audit
  scripts, strict-type checks, no-allocation audit, unsafe inventory
  refresh/check, atomic ordering audit, and `git diff --check`.

## Issue 1: Split options output and diagnostic audit clusters

> **Comment:** Completed. `options-message-output-boundary` is gone from
> allocator source. The option/output unsafe markers are now split across libc
> file symbols, recursion guards, delayed buffering, stderr forwarding,
> registered callback routing, bounded formatting, and option printing IDs.

- [x] Split `options-message-output-boundary` into focused IDs for delayed
      output buffering, default output callback lookup, stderr/file output,
      recursion guard entry/exit, formatted output, error reporting, option
      printing, and assertion failure handling.
- [x] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, evidence rows, and
      release-audit guards so each new option/output ID has separate evidence.
- [x] Keep the output paths allocation-free and no-unwind where the current
      no-allocation and panic-audit guards require it.

Verify: primitive option output tests, no-allocation audit, unsafe audit scripts,
unsafe evidence scripts, full formatting, full clippy, and full tests.

## Issue 2: Split arena registry and purge audit clusters

> **Comment:** Completed. The broad arena registry, purge, and debug render IDs
> are gone from allocator source. Arena registry markers are split across
> count/index lookup, heap arena-page tables, start-index selection,
> suitability, iteration, destroy, and add IDs. Purge markers are split across
> scheduling, free dispatch, direct OS purge, range purge, visitor, expiration
> policy, and collect-guard IDs. Arena debug rendering is split across
> snapshots, page-state reads, buffer writes, chunk rendering, and registry
> walking.

- [x] Split `arena-registry-boundary` by operation: arena count/index lookup,
      heap arena-page table lookup, suitability tests, arena iteration, add,
      destroy, and subproc table access.
- [x] Split `arena-purge-boundary` by operation: purge scheduling, direct purge,
      range purge, purge visitor traversal, elapsed/force policy, OS
      decommit/reset, and global collect guard behavior.
- [x] Include nearby `arena-debug-render`, `arena-slice-boundary`, and
      `arena-slice-allocation-boundary` markers where the same edit surface can
      be split without touching unrelated allocator behavior.

Verify: arena behavior release-audit modules, original static arena tests,
atomic ordering audit, unsafe audit scripts, unsafe evidence scripts, full
formatting, full clippy, and full tests.

## Issue 3: Split allocation entry and raw override audit clusters

> **Comment:** Completed. The broad allocation heap-dispatch, realloc, C++ new,
> and raw override IDs are gone from allocator source. Allocation dispatch is
> split across small, thread-heap zeroing, public malloc, zalloc, and calloc
> IDs. Realloc is split across thread-entry, product, free-on-failure, zeroing,
> and public ABI IDs. C++ new and raw override markers are split by handler,
> retry, product/aligned/realloc, fast override assembly, macOS interpose, and
> Windows CRT families.

- [x] Split `allocation-realloc-entry-boundary` into realloc request parsing,
      reallocn/recalloc product checks, reallocf free-on-failure, zeroing
      variants, heap-specific variants, and public C ABI forwarding IDs.
- [x] Split `allocation-heap-dispatch-boundary` into default thread-heap
      dispatch, borrowed heap dispatch, small allocation fast paths, zeroing
      dispatch, and allocation product/request helpers.
- [x] Split `allocation-cxx-new-boundary` and `raw-macro-override-entry-boundary`
      by ABI family so C++ new/delete, Unix exports, Windows exports, bootstrap
      forwarding, and interpose tables are reviewed independently.

Verify: allocation boundary cases, aligned/compat exports, interposition
exports, raw facade release-audit modules, unsafe audit scripts, unsafe evidence
scripts, full formatting, full clippy, and full tests.

## Issue 4: Split typed page-state and subproc association audit clusters

> **Comment:** Completed. `typed-page-state-domain` and
> `init-subproc-heap-association-boundary` are gone from allocator source. Page
> typestate markers are split across uninit initialization, witness
> construction, active/full/available/all-free transitions, retire/destroy,
> abandoned claim/reclaim, remote-free classification, local thread-heap
> witnesses, and free-owner classification. Subproc association markers are
> split across prefix projection, main-heap loads, main-heap export/current
> lookup, main-heap predicates, and heap visitation.

- [x] Split `typed-page-state-domain` into uninit initialization, active/free
      transitions, available/full transitions, all-free retire/expire,
      abandoned claim/reclaim, foreign/remote-free classification, and
      thread-heap local/borrowed witnesses.
- [x] Split `init-subproc-heap-association-boundary` into subproc prefix/main
      heap lookup, heap-to-subproc lookup, current subproc lookup, main heap
      export, main heap predicates, thread association, and heap visitation IDs.
- [x] Update strict-type and datatype acceptance guards so the typed witness
      transitions remain covered after the audit IDs are split.

Verify: page-state tests, lifecycle/subproc release-audit modules, strict-type
checks, unsafe audit scripts, unsafe evidence scripts, full formatting, full
clippy, and full tests.

## Issue 5: Tighten strict-type allowlist hotspots

The largest strict-type hotspots remain raw-pointer parameters in
`src/allocator/alloc.rs` (47), `src/allocator/page.rs` (44), and
`src/internal/mod.rs` (41), plus bare-domain `usize` entries in
`src/internal/bitmap.rs` (33) and `src/raw/macros.rs` (32). The Loom
proof-model heap-allocation boundary is now exact-symbol scoped.

> **Comment:** Completed. The top raw-pointer and bare-domain `usize` hotspots
> were audited and retained as symbol-level C ABI, translated-C hot-path, bitmap
> traversal, or raw facade boundaries where moving signatures behind wrappers
> would change compatibility shape without removing an unsafe/source boundary.
> The retained Loom proof-model heap-allocation boundary no longer uses a
> wildcard; each modeled test function is covered by an exact symbol.

- [x] Audit the top raw-pointer hotspot families and remove entries that can be
      moved behind existing `ThreadHeapLocal`, `PagePtr`, `HeapPtr`, or
      `BlockPtr` witnesses without adding hot-path allocation or dynamic
      dispatch.
- [x] Split or tighten bare-domain `usize` allowlist entries in bitmap and raw
      facade helpers where existing scalar/domain wrappers can carry the same
      invariants.
- [x] Replace the retained Loom wildcard with exact symbol coverage if the
      checker can model that proof boundary; otherwise keep the wildcard count
      measured and documented.

Verify: strict-type checker, datatype acceptance release-audit modules,
no-allocation audit, full formatting, full clippy, and full tests.

## Issue 6: Split release-audit modules over 400 lines and lower the cap

The recursive module-size guard now enforces 400 lines. The next bottleneck is
the high-300-line band: `allowance_inventory.rs` at 398,
`unsafe_performance/lifecycle_boundaries/thread_tls.rs` at 390,
`interposition_exports/linux_override.rs` at 371,
`unsafe_performance/page_arena_behavior/memid_arena_dispatch.rs` at 367,
`primitive_options.rs` at 323, and `unsafe_performance/thread_stats.rs` at 319.

> **Comment:** Completed. The over-400-line release-audit modules are now thin
> parent dispatchers plus focused child modules for lifecycle/init markers,
> subproc fast paths, allocation/free marker groups, typed helper checks,
> OS/platform wrappers, Unix/WASI/Emscripten platform backends, interposition
> export groups, and unsafe-audit evidence families. The recursive guard now
> enforces the 400-line cap against every release-audit module file.

- [x] Split `process_lifecycle.rs`, `interposition_exports.rs`,
      `unix_wasi_emscripten.rs`, `allocation_free.rs`, and `os_memory.rs` into
      focused modules matching their existing test groups.
- [x] Split `release_policy/security/unsafe_audit_evidence.rs` by evidence
      family so source cluster caps, stale ID rejection, docs evidence, and TSV
      evidence stay independently reviewable.
- [x] Lower the release-audit module cap from 500 to 400 without weakening
      recursive module discovery or TODO measured-count checks.

Verify: release fast-path audit, docs/security/release policy modules,
strict-type checks, unsafe audit scripts, unsafe evidence scripts, full
formatting, full clippy, and full tests.

## Suggested implementation order

1. [x] Split options output and diagnostic audit clusters.
2. [x] Split arena registry and purge audit clusters.
3. [x] Split allocation entry and raw override audit clusters.
4. [x] Split typed page-state and subproc association audit clusters.
5. [x] Tighten strict-type allowlist hotspots.
6. [x] Split release-audit modules over 400 lines and lower the cap.

## Definition of done for this backlog

- [ ] No source audit operation ID has more than 32 source markers.
- [ ] The largest source audit clusters listed above are split into narrower
      review units with separate docs, evidence rows, and release-audit guards.
- [x] Strict-type allowlist hotspots are measured in release-audit coverage, and
      any removed or retained entries have explicit symbol-level rationale.
- [x] Release-audit modules are split enough to lower the module cap to 400
      lines without weakening recursive discovery or measured TODO checks.
- [x] The strict-type wildcard guard either reaches zero retained wildcards or
      remains explicitly documented with current measured evidence.
- [x] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
