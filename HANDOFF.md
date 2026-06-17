# Handoff: Mechanical mimalloc Refactoring

## Current Status
Phases 1 through 4 are complete. Phase 5 (Scalar and Layout Enforcement) is now
**complete across all three action items**:

1. **Allocation shims** — `mi_malloc`/`mi_calloc`/`mi_realloc`/`mi_reallocn` and
   the `mi_heap_*` family construct `LayoutRequest`/`CallocRequest` at the C-ABI
   boundary, propagating typed requests inward and bridging back to `usize` via
   `get()` where the theap layers remain unconverted.
2. **Math helpers** — `mi_page_usable_block_size` returns `UsableSize`;
   `BlockSize::usable()` is branchless (`debug_assert!` + `wrapping_sub` in
   release, preserving the zero-branch mandate); `mi_good_size` routes through
   `RequestedSize`/`GoodSize`.
3. **Arena slices** — `mi_arenas_try_alloc` and `mi_arenas_try_find_free` now
   accept `SliceCount` (`NonZeroU32`), closing a hole where `slice_count == 0`
   would pass the `saturating_mul` size guard. A safe `SliceCount::from_usize`
   constructor was added.

All gates pass: `cargo build --release`, `cargo fmt --check`, `cargo clippy
--all-targets`, `cargo test` (6 unit tests pass). Two pre-existing failures
remain unchanged by this work: `aligned_exports` (assertion at
`alloc_aligned.rs:189`) and two cases in `release_fast_path_audit`
(`default_linux_malloc_override_uses_c_tls_fast_path`,
`free_path_internals_are_not_dynamic_exports`) — both fail identically on the
clean tree and are tracked separately.

## Known follow-ups (not blocking Phase 6)

- The `mi_reallocf`/`mi_heap_reallocf` free-on-failure contract is restored; the
  other `mi_realloc*` entry points keep `p` on failure per POSIX, which is
  correct.
- `GoodSize`/`RequestedSize` at the `mi_good_size` boundary currently carry no
  runtime invariant (constructors accept any `usize`). They are scaffolding for
  the deeper Phase 5 cascade that pushes typed sizes into `_mi_page_malloc` and
  below; not a safety win on their own.
- ~175 release warnings remain, almost entirely unused-import/unused-type noise
  from `core_types` re-exports whose call sites have not yet been migrated. This
  is Phase 6 scope (see below).

## Recommended Next Steps: Phase 6 — Code Cleanup & Linting

1. **Resolve the unused-import / unused-type warning sweep.** Walk the ~175
   release warnings. For each `core_types` re-export:
   - If a call site exists, migrate it to use the typed API.
   - If genuinely unused, remove the re-export (and the type if nothing in
     `core_types` uses it either). Notable known orphans: `free_actions.rs`
     still imports `BlockForFree, ForeignPage, LocalPage` which are now unused
     after the Phase 4 stub deletion — start there.
2. **Delete obsolete C-style aliases** from `src/types.rs` only where fully
   superseded *and* not referenced by `raw`/ffi/compat modules. Per
   `TYPES_SPEC.md` §2.1, the canonical aliases (`mi_page_t`, `mi_memid_t`, etc.)
   **must stay** as the compatibility substrate — do not delete those. Only
   remove aliases that are truly dead.
3. **SAFETY documentation sweep.** Ensure every `unsafe fn` and `unsafe {}`
   block has a `/// # Safety` or `// SAFETY:` comment justifying correctness
   against the strict invariants. Known sites needing notes: the remaining
   `new_unchecked` calls in `arena.rs` and `free.rs`.
4. **Remove patch artifacts** left in the working tree: `src/arena.rs.orig`,
   `src/arena.rs.rej`, `src/internal.rs.orig`, `fix_arena_slices.patch`. These
   are leftover failed-patch attempts and should not be tracked.
5. **Investigate the two pre-existing test failures** (`aligned_exports`,
   `release_fast_path_audit`) if Phase 6 scope allows — they predate the typed
   migration but should be resolved before declaring the refactor complete.

Good luck!
