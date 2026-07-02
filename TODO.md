# TODO: Fresh Rust Idiom Follow-up Backlog

Fresh-eyes review after completing the previous active TODO backlog on
`master` at `33a8f2d`. That pass migrated `src/types/constants.rs`,
`src/allocator/thread_heap.rs`, `src/platform/unix.rs`, `src/platform/os.rs`,
and `src/types/ptr.rs` off the unsafe migration allowlist while preserving raw
ABI layout, thread-heap lifecycle behavior, Unix and OS memory boundaries, typed
pointer layout, and allocator hot-path constraints.

The next pass should keep the same discipline: retire one measurable exception
at a time, prefer local operation-level proof markers over file allowances, and
avoid refactors that change allocator ownership, synchronization, platform ABI,
or allocation fast paths.

Current review evidence:

- `docs/unsafe-migration-allowlist.tsv` has 9 entries: 7 production and
  2 translation files. `ALLOWLIST_ENTRY_CAP` is 9.
- The completed pass moved `src/types/constants.rs` to 8
  `SAFETY[AUDIT:raw-layout-abi-typedefs]` markers while retaining 2
  `SAFETY[AUDIT:raw-auto-trait]` markers for sentinel `Sync` impls, moved
  `src/allocator/thread_heap.rs` to 71
  `SAFETY[AUDIT:thread-heap-lifecycle]` markers, moved
  `src/platform/unix.rs` to 90 `SAFETY[AUDIT:unix-primitive-backend]` markers,
  moved `src/platform/os.rs` to 136 `SAFETY[AUDIT:os-memory-boundary]`
  markers, and moved `src/types/ptr.rs` to 161
  `SAFETY[AUDIT:typed-pointer-accessors]` markers.
- This pass moved `src/allocator/free.rs` to 69
  `SAFETY[AUDIT:free-path-boundary]` markers for free fast-path assembly
  aliases, page/block/user-pointer validation, padding and guarded-block
  metadata, local/remote/abandoned routing, thread-free atomic publication,
  abandoned collect/reclaim/reabandon decisions, usable-size shims, raw C ABI
  free exports, and stack-local tests, and is off the unsafe migration
  allowlist.
- This pass also moved `src/allocator/options.rs` to 76
  `SAFETY[AUDIT:options-callback-boundary]` markers for option descriptors,
  environment lookup, C string handling, output/error callback state, recursion
  guards, diagnostic output, option raw exports, and assert abort shims, and is
  off the unsafe migration allowlist.
- This pass also moved `src/allocator/aligned.rs` to 158
  `SAFETY[AUDIT:aligned-allocation-boundary]` markers for alignment-sensitive
  allocation, offset-aware layout adjustment, guarded and over-allocated paths,
  padding shrink, usable-size checks, tracking hooks, aligned realloc, heap
  borrowing, and raw aligned exports, and is off the unsafe migration allowlist.
- The smallest remaining allowlisted production files with no local markers are
  `src/allocator/stats.rs` at 2,021 lines with 198 `unsafe` hits, and
  `src/raw/macros.rs` at 2,223 lines with 383 `unsafe` hits,
  `src/allocator/init.rs` at 2,339 lines with 333 `unsafe` hits,
  and `src/internal/bitmap.rs` at 2,358 lines with 141 `unsafe` hits.
- The next allowlisted production files after those are
  `src/allocator/alloc.rs` at 2,559 lines with 333 `unsafe` hits,
  `src/allocator/page.rs` at 2,801 lines with 196 `unsafe` hits, and
  `src/allocator/arena.rs` at 3,915 lines with 607 `unsafe` hits.
- The remaining translation allowlist entries are `src/internal/mod.rs` at
  1,533 lines with 163 `unsafe` hits, and `src/platform/windows.rs` at
  2,191 lines with 214 `unsafe` hits.
- `strict-type-allowlist.txt` has 280 entries: 150 raw-pointer-parameter
  entries, 105 bare-domain `usize` entries, 13 pointer/usize round-trip
  entries, 8 metadata-reference entries, 2 atomic-cast entries, and
  2 heap-allocating-type entries. `REMOVE_ENTRY_CAP` remains 0.
- The broadest path-wide strict-type clusters are `src/allocator/aligned.rs`,
  `src/allocator/stats.rs`, `src/allocator/thread_heap.rs`,
  `src/internal/mod.rs`, `src/platform/unix.rs`, and
  `src/platform/windows.rs`, each with 4 path-wide entries.
- The largest unsafe audit marker clusters remain broad enough to split later:
  `SAFETY[AUDIT:typed-pointer-accessors]` has 161 markers,
  `SAFETY[AUDIT:aligned-allocation-boundary]` has 158 markers,
  `SAFETY[AUDIT:page-map-boundary]` has 153 markers,
  `SAFETY[AUDIT:os-memory-boundary]` has 136 markers,
  `SAFETY[AUDIT:internal-atomic-facade]` has 103 markers,
  `SAFETY[AUDIT:unix-primitive-backend]` has 90 markers,
  `SAFETY[AUDIT:typed-pointer-test-fixtures]` has 87 markers,
  `SAFETY[AUDIT:macos-zone-override]` has 83 markers,
  `SAFETY[AUDIT:heap-lifecycle-boundary]` has 80 markers,
  `SAFETY[AUDIT:options-callback-boundary]` has 76 markers,
  `SAFETY[AUDIT:thread-heap-lifecycle]` has 71 markers,
  `SAFETY[AUDIT:raw-posix-compat]` has 70 markers,
  `SAFETY[AUDIT:free-path-boundary]` has 69 markers,
  `SAFETY[AUDIT:platform-tls-primitives]` has 60 markers,
  `SAFETY[AUDIT:arena-heap-page-visitors]` has 56 markers,
  `SAFETY[AUDIT:typed-heap-test-fixtures]` has 55 markers,
  `SAFETY[AUDIT:typed-atomic-domain]` has 51 markers,
  `SAFETY[AUDIT:ffi-libc-helpers]` has 50 markers, and
  `SAFETY[AUDIT:threadlocal-key-manager]` has 50 markers.
- The release audit is still split but has large review units:
  `tests/release_fast_path_audit/unsafe_performance.rs` is 2,462 lines,
  `tests/release_fast_path_audit/release_policy.rs` is 2,146 lines,
  `tests/release_fast_path_audit.rs` is 1,899 lines, and
  `tests/release_fast_path_audit/datatype_acceptance.rs` is 1,131 lines.

## Issue: Free path ownership boundary still uses a file-level unsafe allowance

> **Comment:** `src/allocator/free.rs` owns user-pointer-to-page lookup,
> ownership validation, local and remote free routing, delayed-free publication,
> guard-page handling, double-free checks, padding shrink, abandon handling, and
> raw free/realloc-adjacent shims. This should be migrated before larger
> allocator files because it is a compact hot-path boundary with clear existing
> behavior tests.

- [x] Add local `SAFETY[AUDIT:free-path-boundary]` markers for page lookup,
      block ownership checks, free-list link writes, delayed-free and
      cross-thread publication, guard-page validation, padding shrink, small and
      huge page free paths, and raw public free shims.
- [x] Preserve typed pointer wrappers, page ownership and segment invariants,
      double-free detection, abandoned-page semantics, memory-ordering choices,
      delayed-free retry behavior, usable-size accounting, and the absence of
      allocator allocation or dynamic dispatch on free hot paths.
- [x] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, local free, remote free,
      guarded free, invalid/double-free rejection, huge allocation free, and
      no-allocation critical paths.
- [x] Add matching unsafe audit/evidence entries, remove `src/allocator/free.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: free-path release audit, allocation boundary cases, no-allocation
critical-path tests, unsafe audit scripts, unsafe inventory regeneration/check,
full clippy, and full tests.

## Issue: Options, output, and callback state still use a file-level unsafe allowance

> **Comment:** `src/allocator/options.rs` owns option descriptors, environment
> and string parsing, process-global option mutation, output/error callbacks,
> recursion guards, and raw C option exports. It is smaller than most remaining
> allocator files and has many C-shaped operations that should be classified
> locally instead of hidden behind a file allowance.

- [x] Add local `SAFETY[AUDIT:options-callback-boundary]` markers for raw option
      descriptor access, C string conversion, environment reads, option
      load/store helpers, output/error callback pointer publication, recursive
      message guard access, stderr fallback writes, and raw option exports.
- [x] Preserve option defaults, option name aliases, environment variable
      parsing, atomic ordering and locking, callback ABI signatures, callback
      nullability, recursion behavior, message formatting, and no allocator
      allocation in primitive output paths.
- [x] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, option descriptor shape, raw
      option exports, callback locking, recursion guards, stderr fallback, and
      environment parsing behavior.
- [x] Add matching unsafe audit/evidence entries, remove
      `src/allocator/options.rs` from the unsafe migration allowlist,
      regenerate unsafe inventory, and lower the cap.

Verify: option/callback release audit, option parsing tests, callback/output
tests, unsafe audit scripts, unsafe inventory regeneration/check, full clippy,
and full tests.

## Issue: Aligned allocation boundary still uses a file-level unsafe allowance

> **Comment:** `src/allocator/aligned.rs` owns alignment-sensitive allocation,
> aligned realloc, offset-aware layout adjustment, usable-size preservation,
> over-allocation trimming, and original-allocation pointer recovery. It is a
> hot path, so migration needs explicit marker proof plus existing
> no-allocation/performance evidence before removing the file allowance.

- [x] Add local `SAFETY[AUDIT:aligned-allocation-boundary]` markers for aligned
      allocation entry points, offset and alignment validation, allocation-size
      rounding, guard-page and OS fallback paths, aligned realloc movement,
      usable-size queries, original pointer recovery, and raw aligned exports.
- [x] Preserve alignment and offset contracts, overflow handling, zero-size and
      huge-size behavior, in-place realloc behavior, allocation ownership
      transfer, layout metadata reads/writes, error paths, and no dynamic
      dispatch on aligned allocation hot paths.
- [x] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, alignment/offset edge cases,
      realloc preservation, usable-size behavior, original pointer recovery,
      and benchmark or no-allocation hot-path evidence.
- [x] Add matching unsafe audit/evidence entries, remove
      `src/allocator/aligned.rs` from the unsafe migration allowlist,
      regenerate unsafe inventory, and lower the cap.

Verify: aligned allocation/reallocation tests, allocation boundary cases,
release audit performance checks, unsafe audit scripts, unsafe inventory
regeneration/check, full clippy, and full tests.

## Issue: Stats reporting boundary still uses a file-level unsafe allowance

> **Comment:** `src/allocator/stats.rs` owns per-thread and process stats
> counters, stats merging, formatted stats output, JSON-ish output paths, raw
> output callbacks, and C export shims. It mixes accounting state with C callback
> boundaries, so local operation markers should distinguish counter projection
> from output and callback safety.

- [ ] Add local `SAFETY[AUDIT:stats-reporting-boundary]` markers for stats
      counter access, thread/process stats merging, buffer writes, raw callback
      dispatch, formatted output helpers, JSON field emission, and raw stats
      exports.
- [ ] Preserve counter width and saturation behavior, reset and merge semantics,
      output ordering, callback ABI and nullability, buffer bounds, process
      option interactions, and no allocator recursion from stats output paths.
- [ ] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, stats merge/reset behavior,
      formatted output stability, callback output behavior, and no-allocation
      critical paths.
- [ ] Add matching unsafe audit/evidence entries, remove `src/allocator/stats.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: stats release audit, stats output tests, no-allocation critical-path
tests, unsafe audit scripts, unsafe inventory regeneration/check, full clippy,
and full tests.

## Issue: Raw macro allocation facade still uses a file-level unsafe allowance

> **Comment:** `src/raw/macros.rs` is a raw C compatibility facade for
> allocator entry points, override macros, bootstrap tracking, and C-header-like
> allocation functions. It remains mechanically translated in shape, but the
> unsafe boundary can still be documented with operation markers while preserving
> public macro/API compatibility.

- [ ] Add local `SAFETY[AUDIT:raw-macro-allocation-boundary]` markers for raw
      allocation, calloc, realloc, aligned allocation, free, usable-size,
      override, bootstrap, and C-header compatibility entry points generated or
      shaped by the macro facade.
- [ ] Preserve exported raw names and signatures, feature/cfg behavior,
      bootstrap allocation tracking, weak/override symbol semantics, C allocator
      ownership rules, null and zero-size behavior, panic/unwind containment,
      and no safe wrapper leakage into the raw facade.
- [ ] Add or extend release-audit assertions and focused tests for marker
      coverage, no migration allowlist entry, raw symbol/signature parity,
      override cfg shape, bootstrap behavior, raw free/realloc ownership, and
      allocation boundary behavior.
- [ ] Add matching unsafe audit/evidence entries, remove `src/raw/macros.rs`
      from the unsafe migration allowlist, regenerate unsafe inventory, and
      lower the cap.

Verify: raw macro/facade release audit, raw allocation boundary tests, unsafe
audit scripts, unsafe inventory regeneration/check, full clippy, and full tests.

## Suggested implementation order

1. [x] Migrate `src/allocator/free.rs` off the unsafe migration allowlist.
2. [x] Migrate `src/allocator/options.rs` off the unsafe migration allowlist.
3. [x] Migrate `src/allocator/aligned.rs` off the unsafe migration allowlist.
4. [ ] Migrate `src/allocator/stats.rs` off the unsafe migration allowlist.
5. [ ] Migrate `src/raw/macros.rs` off the unsafe migration allowlist.

## Definition of done for this backlog

- [ ] The unsafe migration allowlist cap is lowered after each migrated file.
- [ ] `src/allocator/free.rs`, `src/allocator/options.rs`,
      `src/allocator/aligned.rs`, `src/allocator/stats.rs`, and
      `src/raw/macros.rs` are covered by operation-level audit markers and
      evidence instead of file-level migration exceptions.
- [ ] Free routing, option/callback behavior, aligned allocation layout, stats
      output semantics, raw macro facade compatibility, and allocator hot-path
      performance constraints remain unchanged.
- [ ] Release-audit TODO guards track the active backlog and measurable
      evidence.
- [ ] Before considering the backlog complete, rescan the codebase with fresh
      eyes, rewrite `TODO.md` with the next set of concrete issues, and set a
      new goal to work through that refreshed backlog.
