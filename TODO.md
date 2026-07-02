# TODO: Strict-Type and Audit Granularity Backlog

Fresh-eyes review after completing the unsafe migration allowlist on `master` at
`da60fc0`. The file-level unsafe migration work is done; the next mechanical
translation debt is now in broad strict-type allowances, broad unsafe operation
IDs, and large release-audit review units.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 0 translation entries and no
  production entries. `ALLOWLIST_ENTRY_CAP` is 0.
- `scripts/check-unsafe-audit.py` covers 82 source files, 5,893 unsafe mentions,
  and 81 operation IDs.
- `strict-type-allowlist.txt` has 280 entries: 150 raw-pointer-parameter
  entries, 105 bare-domain `usize` entries, 13 pointer/usize round-trip
  entries, 8 metadata-reference entries, 2 atomic-cast entries, and
  2 heap-allocating-type entries. `REMOVE_ENTRY_CAP` remains 0.
- 75 strict-type entries still use symbol `*`: 29 raw-pointer-parameter
  entries, 21 bare-domain `usize` entries, 13 pointer/usize round-trip entries,
  8 metadata-reference entries, 2 heap-allocating-type entries, and
  2 atomic-cast entries.
- `scripts/check-strict-types.py` now checks line-pattern findings with an
  item-aware context for free functions, impl methods, associated consts,
  statics, and extern declarations. It still allows deliberate wildcard
  fallback entries for module-level or true ABI boundaries.
- The broadest path-wide strict-type clusters are `src/platform/windows.rs`,
  `src/platform/unix.rs`, `src/internal/mod.rs`,
  `src/allocator/thread_heap.rs`, `src/allocator/stats.rs`, and
  `src/allocator/aligned.rs`, each with 4 path-wide entries.
  `src/platform/wasi.rs`, `src/platform/os.rs`, `src/allocator/init.rs`,
  `src/allocator/free.rs`, and `src/allocator/alloc.rs` each still have
  3 path-wide entries.
- The largest source unsafe audit marker clusters are still too broad:
  `SAFETY[AUDIT:arena-region-boundary]` has 340 markers,
  `SAFETY[AUDIT:init-lifecycle-boundary]` has 277 markers,
  `SAFETY[AUDIT:raw-macro-allocation-boundary]` has 178 markers,
  `SAFETY[AUDIT:allocation-entry-boundary]` has 166 markers,
  `SAFETY[AUDIT:stats-reporting-boundary]` has 161 markers,
  `SAFETY[AUDIT:typed-pointer-accessors]` has 161 markers,
  `SAFETY[AUDIT:aligned-allocation-boundary]` has 158 markers,
  `SAFETY[AUDIT:page-map-boundary]` has 153 markers,
  `SAFETY[AUDIT:os-memory-boundary]` has 136 markers, and
  `SAFETY[AUDIT:windows-primitive-boundary]` has 114 markers.
- The release audit is split, but the largest review units are still heavy:
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,567 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,150 lines,
  `tests/release_fast_path_audit.rs` is 1,935 lines,
  `tests/release_fast_path_audit/datatype_acceptance.rs` is 1,151 lines,
  `tests/release_fast_path_audit/unsafe_performance/platform_primitive_boundaries.rs`
  is 895 lines, and
  `tests/release_fast_path_audit/unsafe_performance/lifecycle_boundaries.rs`
  is 831 lines.

## Issue 1: Make strict-type line-pattern checks item-aware

> **Comment:** The strict-type checker can only narrow some checks today.
> Raw-pointer and bare-domain parameters are symbol-aware, but line-pattern
> checks used file-level `*` allowances. This pass made those checks
> item-aware while preserving deliberate wildcard fallback compatibility for
> true ABI or module-level boundaries.

- [x] Teach `scripts/check-strict-types.py` to identify the containing Rust item
      for line-based findings, including free functions, associated functions,
      extern blocks, statics, consts, and impl methods.
- [x] Use the detected item name when checking metadata-reference,
      pointer-usize-roundtrip, heap-allocating-type, atomic-cast, and
      dynamic-dispatch entries; keep a deliberate fallback for module-level or
      extern-block findings.
- [x] Preserve the existing checker behavior for raw ABI boundaries and
      translated C compatibility shims; do not create false positives for cfg
      branches, test modules, macro definitions, or source comments.
- [x] Add focused checker tests or release-audit assertions that prove new
      line-pattern allowlist entries can be symbol-level and that new broad
      `*` entries need explicit justification.

Verify: `scripts/check-strict-types.py`, release fast-path TODO/datatype audit
tests, full formatting, full clippy, and full tests.

## Issue 2: Narrow platform strict-type wildcard allowances

> **Comment:** Platform primitives are legitimate raw compatibility boundaries,
> but the current wildcard entries hide which functions actually need the raw
> pointer, metadata-reference, pointer/integer, byte-count, and heap-allocating
> allowances.

- [ ] Replace broad strict-type entries for `src/platform/windows.rs` with
      symbol/domain entries for TLS/FLS association, virtual-memory byte counts,
      ETW/metadata callbacks, environment string conversion, and dynamic import
      helpers.
- [ ] Replace broad strict-type entries for `src/platform/unix.rs` with
      symbol/domain entries for TLS association, mmap/madvise byte counts,
      metadata scanning, and alignment/provenance checks.
- [ ] Narrow the matching platform entries in `src/platform/wasi.rs` and
      `src/platform/os.rs` where the checker can now name the responsible
      function or item.
- [ ] Add release-audit guards that reject accidental new platform wildcard
      strict-type entries unless they are explicitly documented ABI shims.

Verify: strict-type checker, platform primitive release-audit tests,
no-allocation audit, full clippy, and full tests.

## Issue 3: Narrow allocator and internal strict-type wildcard allowances

> **Comment:** Several hot allocator files still carry path-wide strict-type
> allowances even after the unsafe migration allowlist was removed. The next
> pass should convert those allowances to named helper boundaries without
> perturbing hot-path code shape.

- [ ] Narrow `src/allocator/aligned.rs` wildcard entries for raw heap/thread-heap
      parameters, guarded block handoff, and alignment/provenance arithmetic to
      the specific allocation and padding helpers that need them.
- [ ] Narrow `src/internal/mod.rs` wildcard entries for page/block helper
      parameters, thread-heap dispatch, metadata references, and packed-pointer
      conversions to named helper items.
- [ ] Narrow `src/allocator/stats.rs` and `src/allocator/thread_heap.rs`
      wildcard entries to the stats aggregation, heap lifecycle, page visitor,
      and area visitor helpers that need raw compatibility.
- [ ] Add release-audit assertions that the targeted files no longer gain broad
      strict-type wildcards while preserving existing no-allocation and assembly
      expectations for allocation/free hot paths.

Verify: strict-type checker, datatype acceptance audit, no-allocation audit,
release fast-path audit, full clippy, and full tests.

## Issue 4: Split the arena audit operation cluster

> **Comment:** `SAFETY[AUDIT:arena-region-boundary]` now covers 340 source
> markers across arena registry state, slice math, OS allocation, abandoned-page
> claiming, heap page tables, purge traversal, and page lifecycle shims. That is
> too broad to review well.

- [ ] Split arena markers into narrower operation IDs for registry/subproc table
      access, slice/range projection, arena allocation/reservation, abandoned
      page claim/reclaim, purge traversal, and page lifecycle handoff.
- [ ] Update `UNSAFE_AUDIT.md`, `docs/unsafe-audit.md`, and
      `docs/unsafe-audit-evidence.tsv` so each new operation ID has distinct
      invariants and evidence.
- [ ] Move the arena release-audit assertions out of the monolithic
      `unsafe_performance.rs` test into focused arena modules that guard marker
      counts, helper shape, and ownership transfer behavior.
- [ ] Preserve arena index bounds, NUMA fields, memid ownership, abandoned-page
      semantics, purge ordering, subproc locking, OS memory interaction, and C
      ABI callback signatures.

Verify: arena/page lifecycle release audits, arena metadata tests, unsafe audit
scripts, unsafe evidence scripts, unsafe inventory regeneration/check, full
clippy, and full tests.

## Issue 5: Split the initialization lifecycle audit cluster

> **Comment:** `SAFETY[AUDIT:init-lifecycle-boundary]` has 277 source markers
> across subproc identity, process init/done, thread init/done, TLS, guarded
> setup, options/stat setup, random reinitialization, CPU feature detection, and
> constructor/destructor hooks. Those are separate invariants.

- [ ] Split init markers into narrower operation IDs for subproc identity and
      main heap access, process lifecycle, thread/TLS lifecycle, guarded/options
      setup, random/CPU initialization, and platform constructor/destructor
      hooks.
- [ ] Update audit docs and evidence rows with separate invariants for each
      lifecycle operation ID.
- [ ] Split lifecycle release-audit assertions so process, thread/TLS, subproc,
      and CPU/random checks are reviewable independently.
- [ ] Preserve idempotent initialization, preloading gates, default heap TLS,
      subproc ID mapping, option/stat initialization order, random reseeding,
      no-unwind behavior, and no hidden allocator allocation before init is
      safe.

Verify: lifecycle release audits, runtime-gate tests, original lifecycle and
override tests, unsafe audit scripts, unsafe evidence scripts, full clippy, and
full tests.

## Issue 6: Shrink release-audit review units and lower line caps

> **Comment:** The release audit is doing useful work, but some modules are now
> large enough that future changes become hard to review. Splitting tests by
> boundary also helps keep later audit and strict-type changes focused.

- [ ] Split `tests/release_fast_path_audit/unsafe_performance.rs` into focused
      modules for allocator allocation/free, arena/page behavior, OS memory, raw
      facade checks, and shared helpers.
- [ ] Split `tests/release_fast_path_audit/release_policy.rs` into policy
      modules that match the documentation areas they guard.
- [ ] Split or reorganize `tests/release_fast_path_audit/datatype_acceptance.rs`
      so strict-type, layout, provenance, callbacks, and page-state acceptance
      checks can be reviewed independently.
- [ ] Lower the release-audit line-count guard after the split so the root and
      module caps catch future growth before files reach the current 2,000-line
      scale.

Verify: release fast-path audit, full formatting, full clippy, and full tests.

## Suggested implementation order

1. [x] Make strict-type line-pattern checks item-aware.
2. [ ] Narrow platform strict-type wildcard allowances.
3. [ ] Narrow allocator and internal strict-type wildcard allowances.
4. [ ] Split the arena audit operation cluster.
5. [ ] Split the initialization lifecycle audit cluster.
6. [ ] Shrink release-audit review units and lower line caps.

## Definition of done for this backlog

- [x] The strict-type checker can enforce symbol-level entries for line-pattern
      findings and has release-audit coverage for that behavior.
- [ ] The targeted platform, allocator, and internal strict-type wildcard
      entries are replaced by named symbol/domain allowances or are explicitly
      documented as true ABI/module boundaries.
- [ ] The arena and initialization operation IDs are split into narrower audit
      IDs with separate docs, evidence, and release-audit guards.
- [ ] The large release-audit modules are split enough to lower the line-count
      caps without weakening coverage.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
