# TODO: Complete `TYPES_SPEC.md` Strict-Type Promise

Scope: finish the internal strict-type migration promised by `TYPES_SPEC.md`.
This TODO replaces the broad safe-API plan. The goal is to make allocator
invariants explicit and, where practical, unrepresentable at compile time while
preserving the raw C ABI and hot-path code shape.

A task is done only when the code, tests, docs, and listed validation are done.
Raw C ABI shims may keep C-shaped pointers and `usize` parameters. Internal
allocator logic below those shims should consume typed wrappers once the matching
domain type exists.

## Completion Rules

- [x] Keep `raw`, `abi`, and exported `extern "C"` shims C-shaped; do not break
  public mimalloc ABI compatibility while migrating internals.
- [x] Every non-ABI internal API must use the strongest available strict type for
  page, block, heap, thread-heap, arena, subproc, memid, bin, size, alignment,
  count, index, flag, encoded-word, and atomic-field domains.
- [x] Any remaining raw pointer or bare `usize` in internal code must be either a
  true C/compat boundary or documented in a strict-type allowlist with a removal
  task.
- [x] Witness types such as `LocalPage`, `ValidatedFree`, and
  `OwnedAbandonedPage` must not be safely forgeable outside their validating
  module.
- [x] Hot-path wrappers must be `#[repr(transparent)]`, `Copy` where applicable,
  `#[inline(always)]`/`#[inline]` as appropriate, and proven to add no extra
  branches or memory traffic.
- [x] Raw layout compatibility must remain proven by layout tests across the
  supported feature matrix.

## Required Validation Set

Run before marking the full strict-type migration complete:

```sh
cargo fmt --check
cargo test --quiet --lib
cargo test --quiet --features unstable-raw-internals --test release_fast_path_audit
cargo test --quiet --features std
scripts/check-layout.sh
scripts/check-raw-facade.sh
scripts/check-ffi-boundary.py
scripts/check-api-inventory.py
scripts/check-safe-boundary.py
scripts/check-strict-types.py
scripts/check-lints.sh
scripts/check-features.sh
scripts/check-abi-symbols.sh
```

For hot-path migrations, also run the release assembly and benchmark checks added
by this TODO and record the decision in `BENCHMARK_RESULTS.md` or a dedicated
strict-type signoff section.

---

## Phase 0: Status, Inventory, And Enforcement

- [x] Add `MIGRATION_PHASES.md` or update `docs/strict-types.md` to stop
  referencing a missing status file; record strict-type phase status there.
- [x] Add `scripts/check-strict-types.py` to enforce the acceptance rules from
  `TYPES_SPEC.md` instead of relying only on source-shape tests.
- [x] Add a strict allowlist file for true C/compat raw signatures; every raw
  pointer or bare-domain `usize` left in internals must be justified there.
- [x] Teach `scripts/check-strict-types.py` to reject internal `*mut mi_page_t`
  parameters outside the allowlist.
- [x] Teach `scripts/check-strict-types.py` to reject internal `*mut mi_block_t`
  parameters outside the allowlist.
- [x] Teach `scripts/check-strict-types.py` to reject internal `*mut mi_heap_t`
  parameters outside the allowlist.
- [x] Teach `scripts/check-strict-types.py` to reject internal `*mut mi_theap_t`
  parameters outside the allowlist.
- [x] Teach `scripts/check-strict-types.py` to reject internal `*mut mi_arena_t`
  parameters outside the allowlist.
- [x] Teach `scripts/check-strict-types.py` to reject internal `*mut mi_subproc_t`
  parameters outside the allowlist.
- [x] Teach `scripts/check-strict-types.py` to reject direct memid union reads
  outside `src/core_types/memid.rs` and raw layout modules.
- [x] Teach `scripts/check-strict-types.py` to reject direct `mi_block_s.next`
  reads/writes outside `src/core_types/freelist_wrappers.rs`.
- [x] Teach `scripts/check-strict-types.py` to reject direct `xthread_id` bit
  operations outside `PageThreadId`/`AtomicPageThreadId` helpers.
- [x] Teach `scripts/check-strict-types.py` to reject direct `xthread_free` bit
  operations outside `ThreadFreeWord`/`AtomicThreadFree` helpers.
- [x] Teach `scripts/check-strict-types.py` to reject pointer-to-atomic casts
  outside `src/core_types/atomic_wrappers.rs`.
- [x] Teach `scripts/check-strict-types.py` to reject broad Rust references to
  allocator metadata such as `&mut *page`, `&*page`, and long-lived references
  to raw page/heap/arena structs outside raw layout tests.
- [x] Teach `scripts/check-strict-types.py` to reject arbitrary pointer-to-`usize`
  round trips outside packed-pointer domains such as `ThreadFreeWord`,
  `EncodedWord`, page-map cells, and alignment/provenance helpers.
- [x] Teach `scripts/check-strict-types.py` to reject heap-allocating Rust types
  (`Vec`, `String`, `Box`, `Arc`, `Mutex`, `OnceLock`) in allocator internals
  unless a documented test/std-only allowlist entry permits them.
- [x] Add size/alignment tests proving every transparent wrapper has the same
  size/alignment as its raw representation.
- [x] Add source tests proving no `core_types` pointer wrapper implements
  `Deref` or `DerefMut`.
- [x] Remove broad/stale dead-code allowances from `core_types` as modules become
  live; retained proof/helper APIs are either used, removed, or tied to narrow
  compile-time reachability references with exact symbols.

---

## Phase 1: Core Module Privacy And Witness Construction

- [x] Make `PagePtr`, `BlockPtr`, `HeapPtr`, `ThreadHeapPtr`, `TldPtr`,
  `ArenaPtr`, `SubprocPtr`, `PageQueuePtr`, `UserPtr`, and `OsMemPtr` fields
  private; expose only checked or unsafe constructors.
- [x] Re-export `PageConstPtr`, `InteriorUserPtr`, `PageStart`, `SliceStart`,
  `ArenaIndex`, and `Alignment` from `core_types::mod` once they are live.
- [x] Make `LocalPage`, `ForeignPage`, `AbandonedPage`, `AbandonedMappedPage`,
  `OwnedAbandonedPage`, `ActivePage`, `AvailablePage`, `FullPage`,
  `RetiredPage`, and `DestroyedPage` fields private.
- [x] Add checked/unsafe constructors for each page-state witness with explicit
  invariant docs.
- [x] Make `ValidatedFree::new` unsafe or private; expose only validation-backed
  construction from the pointer-validation module.
- [x] Make `BlockForFree::new` unsafe or private; expose only construction after
  pointer lookup, unalignment, and guarded-block handling.
- [x] Make `AllocatedBlock` and `FreeBlock` fields private; add constructors
  that encode when a block is allocated versus free-list owned.
- [x] Make `ThreadHeapLocal` field private; construct it only through
  `ThreadHeapPtr::try_local` or initialization/reclaim code that proves locality.
- [x] Make `NormalPageQueue`, `FullPageQueue`, and `HugePageQueue` fields
  private; construct them only through queue lookup helpers that prove kind.
- [x] Make `MemId<K>::raw` private; add `into_raw()` for construction paths and
  `as_raw()` only where needed for raw ABI forwarding.
- [x] Make `Padding`, `PaddingCanary`, and `PaddingDelta` fields private where
  external mutation can bypass bounds or canary invariants.
- [x] Add compile-time tests or source tests proving forged witness tuple
  construction is no longer possible outside `core_types`.

---

## Phase 2: Scalar Domain Completion

- [x] Fix `LayoutRequest::malloc(0)` so `padded` is based on effective size plus
  padding, not raw requested size; release/no-padding builds must accept zero-size
  C allocation inputs correctly.
- [x] Add `FullBlockSize(NonZeroUsize)` and use it where block size includes all
  allocator metadata/padding.
- [x] Add `AlignmentOffset(usize)` and migrate aligned-at offset parameters away
  from bare `usize`.
- [x] Add `WordCount(usize)` and migrate word-count calculations and APIs.
- [x] Add `PageQueueCount(usize)` and use it for `mi_page_queue_t.count` access.
- [x] Add `PageCapacity(u16)` with constructors checking `used <= capacity <= reserved`.
- [x] Add `UsedBlockCount(u16)` and use it for page `used` reads/writes.
- [x] Add `ReservedBlockCount(u16)` and use it for page `reserved` reads/writes.
- [x] Add `RetireExpire(u8)` and use it for page retirement countdown handling.
- [x] Add `NumaNode(i32)` and migrate NUMA affinity APIs and arena fields.
- [x] Add `ThreadSeq(usize)`, `HeapSeq(usize)`, and `SubprocSeq(usize)` wrappers
  for sequence counters.
- [x] Add `Heartbeat(u64)` and migrate thread-heap heartbeat/deferred-free APIs.
- [x] Add `OsPageSize(NonZeroUsize)` and use it for OS page-size values.
- [x] Add `CommittedBytes(usize)` and `ReservedBytes(usize)` usage for page,
  arena, and OS memory accounting.
- [x] Add `PageMapIndex(usize)` and `PageMapSubIndex(usize)` with bounds checks.
- [x] Move or re-export `EncodedWord(usize)` as the single encoded-pointer word
  domain type.
- [x] Add `Errno(c_int)` and migrate internal error-message/reporting helpers.
- [x] Harden `Bin::new` so safe construction rejects values `>= MI_BIN_COUNT`;
  add `new_unchecked` for proven hot paths.
- [x] Harden `StatsBin::new` so safe construction rejects values `> MI_BIN_HUGE`;
  add `new_unchecked` for proven hot paths.
- [x] Harden `DirectBin::new` so safe construction rejects values
  `>= MI_PAGES_DIRECT`; add `new_unchecked` for proven hot paths.
- [x] Harden `ArenaIndex::new` so safe construction rejects values
  `>= MI_MAX_ARENAS`.
- [x] Replace `MaybeAlignment::new` with checked construction that accepts only
  `0` or power-of-two alignment; add `new_unchecked` for proven raw compatibility
  inputs.
- [x] Add checked arithmetic helpers for `RequestedSize`, `EffectiveSize`,
  `PaddedSize`, `BlockSize`, and `UsableSize`; overflow must return typed errors.
- [x] Migrate `src/page.rs` bin helpers to return checked `Bin`/`StatsBin`
  values and use `new_unchecked` only after a local proof.
- [x] Migrate `src/internal.rs` direct-bin helpers to checked `DirectBin` values.
- [x] Migrate `src/free.rs` usable-size helpers to typed `UsableSize` throughout
  their internal call chain.
- [x] Migrate `src/arena.rs` slice-count, committed-byte, and reserved-byte
  parameters to typed wrappers.
- [x] Migrate `src/page_map.rs` page-map indexes and sub-indexes to
  `PageMapIndex`/`PageMapSubIndex`.
- [x] Migrate `src/options.rs` and stats/error-reporting helpers to `Errno` where
  they currently pass raw errno values internally.
- [x] Add scalar-domain unit tests for every constructor, including overflow,
  zero, upper-bound, and unchecked hot-path cases.

---

## Phase 3: Pointer Domain Completion

- [x] Add field-access methods on `PagePtr` for hot fields using `addr_of!`,
  `addr_of_mut!`, `read`, and `write`; do not create broad `&mut mi_page_t` refs.
- [x] Add `PagePtr::block_size`, `set_block_size`, `used`, `set_used`,
  `capacity`, `reserved`, `free`, `set_free`, `local_free`, and `set_local_free`
  typed field helpers.
- [x] Add `PagePtr::thread_id_cell()` returning `AtomicPageThreadId`.
- [x] Add `PagePtr::thread_free_cell()` returning `AtomicThreadFree`.
- [x] Add `PagePtr::keys()` and `PagePtr::set_keys()` that compile away when
  keys are disabled.
- [x] Add `BlockPtr` helpers for free-list link access that delegate to
  `BlockLink`/`EncodedPtr`.
- [x] Add `HeapPtr` helpers for thread-heap, subproc, arena, and stats fields.
- [x] Add `ThreadHeapPtr` helpers for tld, heap, heartbeat, page queues, direct
  page cache, and stats fields.
- [x] Add `TldPtr` helpers for thread id, stats, arena, and OS thread-local data.
- [x] Add `ArenaPtr` helpers for slice count, index, memid, subproc, parent,
  bitmaps, page table, and purge fields.
- [x] Add `SubprocPtr` helpers for sequence, arena count, thread count, heap
  count, lock, and main heap fields.
- [x] Add `PageQueuePtr` helpers for first, last, count, and block-size fields.
- [x] Add `UserPtr` and `InteriorUserPtr` conversion helpers that preserve
  provenance where possible and keep nullability explicit.
- [x] Add `PageStart`, `SliceStart`, and `OsMemPtr` arithmetic helpers with
  checked offset APIs.
- [x] Replace broad raw metadata references with wrapper field helpers using
  `addr_of!`, `addr_of_mut!`, `read`, and `write`; do not create long-lived
  `&mi_*` or `&mut mi_*` references to aliasable allocator metadata.
- [x] Centralize pointer-to-integer casts in typed packed-pointer/provenance
  helpers; ordinary pointer arithmetic must use pointer wrapper methods instead
  of ad-hoc `as usize` round trips.
- [x] Replace ad-hoc `NonNull::new_unchecked` in non-hot code with checked
  pointer wrapper constructors returning typed errors or `Option`.
- [x] Add pointer-wrapper tests proving `Option<Wrapper>` is pointer-sized for
  every `NonNull` wrapper used in hot paths.

---

## Phase 4: Allocation Request Types

- [x] Split `LayoutError` for internal layout construction from the public safe
  API layout error, or explicitly document and test the shared type boundary.
- [x] Make `LayoutRequest` constructors the only internal path that computes
  `effective + MI_PADDING_SIZE`.
- [x] Add `LayoutRequest::from_effective_unchecked` for hot paths that already
  proved zero-size normalization.
- [x] Change `LayoutRequest::aligned` and `aligned_at` to use `Alignment` and
  `AlignmentOffset` internally.
- [x] Add `CallocRequest::new` as the only internal multiply path for allocation
  count times item size; remove or wrap duplicate overflow helpers.
- [x] Add `AllocProduct` or equivalent typed product for non-calloc count*size
  helpers such as `mallocn`/`reallocn`.
- [x] Replace `ReallocRequest { old: Option<OldAllocation> }` with constructors
  that distinguish `realloc(null, size)` from resizing an existing allocation.
- [x] Make `OldAllocation::new` private/unsafe and construct it only after
  pointer-to-page validation.
- [x] Add `PreserveBytes` computed as `min(old.usable, new.requested)` and use it
  in realloc copy/zero-tail paths.
- [x] Make `MallocGenericFlags::from_raw` unsafe or validate that bit 0 is only
  zeroing and high bits are a valid huge-alignment payload.
- [x] Migrate `src/alloc.rs` generic allocation paths to pass `LayoutRequest`
  instead of raw size once they cross the C ABI boundary.
- [x] Migrate `src/alloc_aligned.rs` aligned allocation paths to pass
  `LayoutRequest`, `Alignment`, and `AlignmentOffset` after ABI boundary parsing.
- [x] Migrate `src/page.rs::_mi_malloc_generic_typed` to accept typed
  `ThreadHeapLocal` or `ThreadHeapPtr` plus `LayoutRequest` and
  `MallocGenericFlags`.
- [x] Add tests for zero-size malloc, zalloc, aligned, aligned-at, calloc
  overflow, realloc null, realloc existing, and huge-alignment flag packing.

---

## Phase 5: Page Flags, Thread IDs, And Free Classification

- [x] Add checked `ThreadId::new` that rejects zero and ids with page-flag bits
  set; keep `new_unchecked` only for proven raw thread-id reads.
- [x] Add `PageFlags::new` that rejects bits outside `MI_PAGE_FLAG_MASK`.
- [x] Add `PageThreadId::from_parts(owner, flags)` so callers do not manually OR
  owner and flag bits.
- [x] Add `PageThreadId::classify_free_path(current) -> FreePathKind` preserving
  the current XOR branch shape.
- [x] Audit the existing `PageThreadId` method set (`owner`, `flags`,
  `is_abandoned`, `is_abandoned_mapped`, `with_flags`, `with_owner`) and
  `XThreadDelta` classifiers (`is_local_fast`, `needs_local_slow`,
  `is_remote_plain`) against `TYPES_SPEC.md` §6.2/§6.3; mark the already-live
  methods as done in `docs/strict-types.md` so the Phase 19 signoff can point at
  them rather than treating them as outstanding.
- [x] Replace direct local/remote free-path comparisons with `FreePathKind` in
  cold code; keep fast path branch-equivalent.
- [x] Remove direct flag-bit mutation from `src/internal.rs`; use
  `AtomicPageThreadId` or `PageThreadId` methods.
- [x] Remove direct flag-bit mutation from `src/page.rs`; use
  `AtomicPageThreadId` or `PageThreadId` methods.
- [x] Remove direct flag-bit mutation from `src/arena.rs`; use
  `AtomicPageThreadId` or `PageThreadId` methods.
- [x] Fix `PagePtr::try_abandoned` so it does not require a current thread id;
  abandoned classification is independent of the current thread.
- [x] Ensure abandoned-mapped classification is represented distinctly by
  `AbandonedMappedPage`, not collapsed into ordinary `AbandonedPage` where the
  distinction matters.
- [x] Add tests proving owner/flag round trips, abandoned-mapped handling, and
  branch-equivalent XOR classification.

---

## Phase 6: Page Ownership And Lifecycle Typestate

- [x] Implement `Page<Uninit>::initialize(ThreadHeapLocal) -> ActivePage` as the
  only typed constructor for newly initialized active pages.
- [x] Implement `ActivePage::available() -> Option<AvailablePage>` based on the
  page `free` head.
- [x] Implement `AvailablePage::pop_free_block` returning `(AllocatedBlock,
  ActivePage)` or an equivalent zero-cost hot-path pair.
- [x] Implement `ActivePage::to_full(NormalPageQueue, FullPageQueue) -> FullPage`
  and move raw full-flag manipulation inside it.
- [x] Implement `FullPage::unfull(FullPageQueue, NormalPageQueue) -> ActivePage`
  and move raw full-flag manipulation inside it.
- [x] Implement `ActivePage::mark_all_free() -> Option<AllFreePage<Active>>` with
  debug assertions for the all-free invariant.
- [x] Implement `AllFreePage<Active>::retire() -> RetiredPage` for page-retire
  transitions.
- [x] Implement `RetiredPage::expire() -> DestroyedPage` for free-to-arena after
  countdown expiration.
- [x] Implement `PageForAbandon` transitions so active/full page abandonment is
  represented before raw abandonment writes occur.
- [x] Implement `AbandonedPage::claim_ownership() -> Option<OwnedAbandonedPage>`
  using `AtomicThreadFree::claim_ownership`.
- [x] Implement `OwnedAbandonedPage::reclaim(ThreadHeapLocal) -> ActivePage` as
  the only typed reclaim transition.
- [x] Implement `OwnedAbandonedPage::try_reabandon_to_mapped()` returning either
  `AbandonedMappedPage` or unchanged ownership.
- [x] Implement `AllFreePage<_>::free_to_arena(ThreadHeapLocal) -> DestroyedPage`
  as the only typed arena-free transition.
- [x] Replace direct construction of `ActivePage`, `FullPage`, `AllFreePage`, and
  `OwnedAbandonedPage` in `src/page.rs` with transition methods or unsafe
  constructors at proof boundaries.
- [x] Replace direct construction of page-state witnesses in `src/free.rs` with
  validation/classification outputs.
- [x] Replace direct construction of page-state witnesses in `src/arena.rs` with
  transition methods or unsafe constructors at proof boundaries.
- [x] Add typestate tests for each lifecycle transition and compile-fail/source
  tests for invalid transitions.

---

## Phase 7: Block, Guarded Block, And Free-List Types

- [x] Move `AllocatedBlock`, `FreeBlock`, `GuardedBlock`, and `BlockForFree` into
  a single `core_types::block` module or document the current split.
- [x] Make `GuardedBlock` construction possible only after guarded-pointer
  detection.
- [x] Add `BlockLink::read_from_raw` that returns `FreeList` or `Guarded` based
  on the current mode instead of always constructing `Guarded` for tag reads.
- [x] Move all `mi_block_s.next` raw reads/writes in `src/internal.rs` behind
  `EncodedPtr`, `BlockLink`, or free-list wrappers.
- [x] Move all `mi_block_s.next` raw reads/writes in `src/alloc.rs` behind
  `BlockLink`.
- [x] Move all `mi_block_s.next` raw reads/writes in `src/alloc_aligned.rs`
  behind `BlockLink`.
- [x] Move all `mi_block_s.next` raw reads/writes in `src/free.rs` behind
  `BlockLink` or free-list wrappers.
- [x] Change `AvailableFreeList::pop` to consume an `AvailablePage` witness that
  proves `free != null` without relying on external comments.
- [x] Change `LocalFreeList::push` to require `FreeBlock` that can only come from
  `BlockForFree` after validation.
- [x] Add `RemoteFreeList` construction only from `AtomicThreadFree` detach/claim
  operations.
- [x] Add `ThreadFreeSnapshot` accessors or remove it if no typed path needs it.
- [x] Add tests for encoded pointer round trips, guarded tags, available pop,
  local push, remote collect, and low-bit owner packing.

---

## Phase 8: Atomic Field Wrappers

- [x] Replace unbounded `&'static Atomic*` returns in `AtomicUsizeField`,
  `AtomicI64Field`, and `AtomicPtrField` with operation methods that keep the
  unsafe reference local to each operation.
- [x] Document alignment, aliasing, and atomic-access requirements once per
  atomic field wrapper.
- [x] Add `AtomicPageThreadId::load`, `store_owner`, `compare_exchange_owner`,
  and `set_flags` helpers for all page owner/flag operations.
- [x] Add `AtomicThreadFree::push_remote_acq_rel`, `collect_acq_rel`,
  `claim_acq_rel`, and `release_acq_rel` names matching `TYPES_SPEC.md`.
- [x] Reconcile `TYPES_SPEC.md` §8.5 `ThreadFreeCell(*mut usize)` with §13.2
  `AtomicThreadFree`: either document `AtomicThreadFree` as the single cell
  type subsuming `ThreadFreeCell` and the `compare_exchange_push`/
  `claim_ownership`/`release_ownership` method names, or add a `ThreadFreeCell`
  alias plus those methods. The remote free CAS loop must call the reconciled
  typed cell rather than raw atomic primitives.
- [x] Migrate `src/internal.rs` thread-free helpers to call `AtomicThreadFree`
  and `ThreadFreeWord`; keep raw helper names only as compatibility shims.
- [x] Migrate `src/page.rs` thread-free collection to `AtomicThreadFree` methods
  and typed `RemoteFreeList` values.
- [x] Migrate `src/free.rs` remote free CAS loop to `AtomicThreadFree` methods
  without manual raw word reconstruction.
- [x] Migrate arena abandoned-count atomics to `AtomicAbandonedCount` with typed
  bin indexes.
- [x] Migrate arena page-table atomics to `AtomicArenaPage` with typed arena/page
  map indexes.
- [x] Reconcile `TYPES_SPEC.md` §11.3 `ArenaPageTablePtr`, `AbandonedCountCell`,
  and `ArenaPageCell` with the §13.2 `Atomic*` wrappers: either document them as
  aliases for `AtomicArenaPage`/`AtomicAbandonedCount`/`AtomicPtrField` or add
  explicit adoption bullets so the staged `src/core_types/arena.rs` wrappers do
  not remain orphaned.
- [x] Migrate thread-heap refcount atomics to a typed refcount field wrapper.
- [x] Migrate internal atomic compatibility call sites away from
  `mi_memory_order_e`; C-compatible memory-order enums must stay in raw/atomic
  facade code only.
- [x] Use `core_types::MemoryOrder`/`core::sync::atomic::Ordering` for all
  internal atomic wrapper APIs.
- [x] Add tests for load/store/fetch/CAS wrappers and failure-updates-expected
  behavior.

---

## Phase 9: MemId Typed Model

- [x] Make `MemId::view` return `Unknown` instead of using
  `NonNull::new_unchecked` when meta or arena union pointers are null.
- [x] Make `SliceRange::from_raw` return `Option`/`Result`; invalid zero count or
  overflow must not construct a typed arena memid view.
- [x] Add typed `MemId<HeapMainMem>` constructor for main-heap memory ids.
- [x] Add typed remap constructor for `MemId<OsMem>` with `OsMemKind::Remap`.
- [x] Preserve `initially_committed`, `initially_zero`, and `pinned` fields in
  `MemId<ArenaMem>::arena` instead of hardcoding defaults where callers know
  them.
- [x] Add `ExternalMemId` to the formal spec docs and ensure its fields are
  covered by tests.
- [x] Migrate raw `MI_MEM_META` / `mem.meta` construction (block index, block
  count, meta page) to `MemId<MetaMem>::meta` + `into_raw()`, covering the
  `MetaMemId`/`BlockIndex` surface from `TYPES_SPEC.md` §12.2/DT-7; do not leave
  the meta path silently skipped alongside the OsMem/ArenaMem/HeapMain paths.
- [x] Replace raw memid constructors in `src/internal.rs` with typed constructors
  and `into_raw()`.
- [x] Replace raw memid constructors in `src/arena.rs` with typed constructors
  and `into_raw()`.
- [x] Replace raw memid constructors in `src/os.rs` with typed constructors and
  `into_raw()`.
- [x] Replace raw memid constructors in `src/init.rs` with typed constructors and
  `into_raw()`.
- [x] Remove or wrap `internal::mi_memid_is_os`, `mi_memid_needs_no_free`, and
  `mi_memid_arena` so arbitrary union inspection goes through `MemId::view`.
- [x] Add tests for every known memkind, unknown memkind, null union arms,
  invalid arena slice counts, and typed constructor round trips.

---

## Phase 10: Arena, Slice, And Page-Map Types

- [x] Make `SliceRange::new(start, count, arena)` validate
  `start + count <= arena.slice_count`.
- [x] Add `SliceRange::end()` and checked addition helpers to prevent open-coded
  slice arithmetic.
- [x] Use `SliceIndex` for every arena slice start parameter after C ABI parsing.
- [x] Use `SliceCount` for every arena slice count parameter after C ABI parsing.
- [x] Use `ArenaIndex` for every arena index parameter after C ABI parsing.
- [x] Add checked conversion helpers for all raw `u32` slice fields; remove direct
  `as u32` truncation outside `core_types::arena`/`memid`.
- [x] Add `ArenaAllocation { arena, range, memid }` and use it for arena-backed
  OS allocation returns.
- [x] Add checked constructors for `SharedArena`, `ExclusiveArena`, and
  `ParentArena` based on raw arena state.
- [x] Migrate `src/arena.rs::mi_arena_slice_start` to `ArenaPtr` + `SliceIndex`.
- [x] Migrate `src/arena.rs::mi_arena_page_at_slice` to `ArenaPtr` + `SliceIndex`.
- [x] Migrate `src/arena.rs::mi_arena_schedule_purge` to `ArenaPtr` +
  `SliceRange`.
- [x] Migrate `src/arena.rs::mi_arena_purge` to `ArenaPtr` + `SliceRange`.
- [x] Migrate `src/arena.rs::mi_arena_try_purge_range` to `ArenaPtr` +
  `SliceRange`.
- [x] Migrate `src/arena.rs::mi_arena_try_alloc_at` to `ArenaPtr` +
  `SliceCount`/`SliceRange`.
- [x] Migrate `src/arena.rs::mi_arenas_try_find_free` to typed arena/subproc
  indexes and slice counts.
- [x] Migrate `src/arena.rs::mi_arenas_try_alloc` to typed slice count and NUMA
  node values.
- [x] Migrate `src/arena.rs::_mi_arenas_alloc_aligned` and `_mi_arenas_alloc` to
  typed size/alignment/slice domains below the ABI boundary.
- [x] Add `RegisteredPageRange` witness for page-map registration.
- [x] Change pointer-to-page lookup so trusted `PagePtr` results require a
  registered page range witness where applicable.
- [x] Migrate `src/page_map.rs::mi_page_map_ensure_committed` to
  `PageMapIndex` + `SliceCount`.
- [x] Migrate `src/page_map.rs::mi_page_map_set_range_prim` to `PagePtr` +
  `PageMapIndex` + `PageMapSubIndex` + `SliceCount`.
- [x] Migrate `src/page_map.rs::mi_page_map_set_range` to typed page-map indexes
  and slice count.
- [x] Migrate `src/page_map.rs::_mi_page_map_register` and `_unregister` to
  `PagePtr` + `SliceCount` below the ABI boundary.
- [x] Add tests for arena slice range bounds, page-map index bounds, registration
  witnesses, and no lossy integer casts.

---

## Phase 11: Heap, Thread-Heap, TLD, And Subproc Types

- [x] Expand `core_types::heap` from staged scaffold to live wrappers for
  `MainHeap`, `OwnedHeap`, and `BorrowedHeap`.
- [x] Add checked constructors proving `MainHeap` is never deleted through
  `mi_heap_delete`.
- [x] Add checked constructors proving `OwnedHeap` is eligible for delete/destroy
  ownership paths.
- [x] Add checked constructors for `BorrowedHeap` from allocation/raw lookup
  contexts.
- [x] Add `ThreadHeapLocal`, `ThreadHeapBorrowed`, and `ThreadHeapState` APIs that
  encode initialized/main/deleted state.
- [x] Add typed `TldPtr` accessors for thread id, subproc, stats, and arena data.
- [x] Add `SubprocSeq`, `ArenaCount`, `ThreadCount`, and `HeapCount` wrappers for
  subproc fields.
- [x] Add `AtomicGuard<'a>` and migrate atomic guard-scope enter/leave code to
  release through `Drop`.
- [x] Add `OnceGuard<'a>` and migrate once-scope enter/leave code to release
  through `Drop`.
- [x] Add `MiLockPtr`/`MiLockGuard` use for subproc and options locks where
  currently open-coded.
- [x] Migrate `src/heap.rs::mi_heap_theap` to `HeapPtr`/`ThreadHeapPtr` typed
  internals.
- [x] Migrate `src/heap.rs::mi_heap_set_numa_affinity` to `HeapPtr` + `NumaNode`
  below the ABI shim.
- [x] Migrate `src/heap.rs::mi_heap_free`, `mi_heap_delete`,
  `_mi_heap_force_destroy`, and `mi_heap_destroy` to `OwnedHeap`/`MainHeap`
  typed deletion paths.
- [x] Migrate `src/heap.rs::mi_heap_check_owned` to `BorrowedHeap`/`OwnedHeap`
  typed ownership checks.
- [x] Migrate `src/theap.rs::_mi_theap_init` to typed heap/thread-heap/tld
  wrappers.
- [x] Migrate `src/theap.rs::_mi_theap_create` to return `ThreadHeapPtr` or
  `ThreadHeapLocal` below the raw heap shim.
- [x] Migrate `src/theap.rs::_mi_theap_incref`, `_mi_theap_decref`, and
  `_mi_theap_delete` to typed refcount and thread-heap state APIs.
- [x] Migrate `src/init.rs` subproc helpers to `SubprocPtr`, `MainSubproc`, and
  `BorrowedSubproc` wrappers.
- [x] Migrate `src/os.rs` subproc memory-free helpers to typed subproc/memid
  inputs where they are not ABI boundaries.
- [x] Add tests proving main heap cannot enter owned deletion helpers and deleted
  thread heaps cannot be used through typed local APIs.

---

## Phase 12: Page Queue Types

- [x] Add checked constructors for `NormalPageQueue`, `FullPageQueue`, and
  `HugePageQueue` that inspect queue identity/block size.
- [x] Add `QueuedPage<NormalQueue>` and `QueuedPage<FullQueue>` construction when
  a page is actually linked in a queue.
- [x] Change full-page unfull logic to consume `QueuedPage<FullQueue>` instead of
  raw `FullPage` + guessed queues.
- [x] Change normal dequeue/free-to-arena logic to consume `QueuedPage<NormalQueue>`
  or `AllFreePage<Active>` as appropriate.
- [x] Make `PageQueue<K>` field private and expose only kind-specific methods.
- [x] Migrate `src/page.rs::mi_page_queue_enqueue_from` to typed queues and page
  witnesses only; keep raw helper private.
- [x] Migrate `src/page.rs::mi_page_queue_enqueue_from_full` to typed queues and
  page witnesses only; keep raw helper private.
- [x] Migrate `src/page.rs::mi_page_queue_remove`, `mi_page_queue_push`,
  `mi_page_queue_push_at_end`, and `mi_page_queue_move_to_front` to typed queue
  wrappers below their compatibility boundaries.
- [x] Migrate `src/page.rs::_mi_page_queue_is_valid` to typed queue wrappers.
- [x] Add tests proving full queues accept only `FullPage` and normal queues
  accept only active non-full pages with matching block size.

---

## Phase 13: Padding, Guarded Allocation, And Tracking Types

- [x] Add `BlockTag` read/write APIs that distinguish aligned and guarded tags
  without raw word comparisons at call sites.
- [x] Add `GuardedConfig` under `feature = "guarded"` with typed min/max/sample
  rate/sample count fields.
- [x] Add `GuardedDecision` and migrate guarded allocation selection logic away
  from raw booleans.
- [x] Add `TrackMode` that resolves Valgrind/ASan/disabled at compile time.
- [x] Migrate `src/alloc.rs::mi_block_ptr_set_guarded` to `GuardedBlock`,
  `BlockTag`, and typed object size wrappers.
- [x] Migrate `src/free.rs::mi_block_unguard` and `mi_block_check_unguard` to
  `GuardedBlock`/`BlockForFree` inputs.
- [x] Change `PaddingDelta::for_usable` to reject values that do not fit `u32`
  instead of blindly casting after debug assertion.
- [x] Change padding canary creation to accept `PagePtr` and `BlockPtr` instead
  of raw `*const c_void` where the caller has typed values.
- [x] Add tests for guarded tags, guarded pointer unprotection witnesses,
  padding-delta overflow, and feature-disabled key behavior.

---

## Phase 14: Callback, No-Std, And No-Allocation Type Boundaries

- [x] Add `src/core_types/callbacks.rs` for internal callback pointer/context
  wrappers promised by `TYPES_SPEC.md`.
- [x] Keep public/raw C callback typedefs in `src/types.rs`/`src/abi.rs`; do not
  replace C ABI callback signatures with Rust trait or wrapper types.
- [x] Add typed wrappers for output, error, deferred-free, heap-visit, and
  thread-local claim callback function pointers where they are stored or routed
  internally.
- [x] Add typed wrappers for callback context pointers so allocator internals do
  not pass arbitrary `*mut c_void` except at raw trampoline boundaries.
- [x] Migrate deferred-free callback storage and invocation to typed callback
  wrappers while preserving no-allocation and reentry constraints.
- [x] Migrate error/output callback storage and invocation to typed callback
  wrappers while preserving raw C ABI behavior.
- [x] Migrate heap/block visitor callbacks to typed internal wrappers below the
  exported raw visitor APIs.
- [x] Migrate thread-local claim callbacks to typed callback wrappers below the
  raw thread-local API.
- [x] Add source checks proving callback trampolines do not allocate, do not
  panic/unwind across C ABI, and keep raw callback typedefs at the raw boundary.
- [x] Add `no_std` source checks proving strict-type modules do not introduce
  `std`, heap allocation, formatting allocation, or synchronization primitives
  into optimized allocator internals.
- [x] Add tests or source checks proving strict-type wrappers do not allocate in
  allocation/free hot paths.

---

## Phase 15: User Pointer Validation And Free Path

- [x] Make `PointerLookup::Valid` construction private to validation helpers.
- [x] Add `PointerLookup::prepare_free()` that returns `Option<ValidatedFree>`
  and performs guarded-block unprotection exactly once.
- [x] Change `mi_validate_block_from_ptr` to take `PagePtr`/`UserPtr` internally
  and return `PointerLookup` without raw pointer recomputation.
- [x] Change `mi_validate_pointer` to produce `PointerLookup` using typed page
  lookup results and typed pointer-location values.
- [x] Change `mi_block_for_free_from_lookup` to live in `core_types::validation`
  or make it an unsafe proof boundary with documented invariants.
- [x] Change `mi_route_free` so `ValidatedFree` is the only value that can enter
  `mi_free_validated`.
- [x] Change `mi_free_validated` to use `PageThreadId::classify_free_path` and
  page ownership witnesses instead of direct witness construction.
- [x] Change `mi_free_ex_slow` to consume `PageFreeOwner` or `FreePathKind` and
  construct local/foreign witnesses only through checked classification.
- [x] Change `_mi_free_generic`, `mi_free_generic_local`, and `mi_free_generic_mt`
  to be raw compatibility shims that immediately build typed validation values.
- [x] Migrate `mi_abandoned_page_try_free`, `mi_abandoned_page_try_reclaim`,
  `mi_abandoned_page_unown_from_free`, and `mi_free_try_collect_mt` to
  `AbandonedPage`/`OwnedAbandonedPage` typed inputs.
- [x] Add tests for null, invalid, bad-alignment, interior pointer, guarded
  pointer, local free, remote free, abandoned free, and double-free paths.

---

## Phase 16: File-By-File Raw Signature Migration

Each item keeps public/exported C functions raw if required, but adds or changes
the internal implementation below it to typed inputs. Each checkbox names one
function or one narrow compatibility pair.

### `src/alloc.rs`

- [x] Fix `*mut mi_page_t` in `_mi_page_malloc_zero_typed` by taking `AvailablePage`/`ActivePage` and `PaddedSize` internally.
- [x] Keep `_mi_page_malloc_zero` as the raw C-shaped shim and immediately build typed page/padded witnesses before dispatching.
- [x] Fix `*mut mi_page_t` in `memset_nonhuge` by taking `PagePtr` and `UserPtr`, or document it as a raw byte-fill compatibility helper.
- [x] Fix `*mut mi_block_t` in `mi_block_ptr_set_guarded` by taking `BlockPtr` and returning `GuardedBlock`/`UserPtr`.
- [x] Fix `*mut mi_theap_t` in `mi_theap_strdup` by taking `ThreadHeapLocal` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_strndup` by taking `ThreadHeapLocal` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_malloc_small_zero_nonnull` by taking `ThreadHeapLocal` after the ABI/lazy-init boundary.
- [x] Fix `*mut mi_theap_t` in `mi_theap_malloc_generic` by taking `ThreadHeapPtr` plus `LayoutRequest`/`MallocGenericFlags`.
- [x] Fix `*mut mi_theap_t` in `mi_theap_malloc_small_zero` by splitting lazy-init raw handling from typed `ThreadHeapLocal` dispatch.
- [x] Fix `*mut mi_theap_t` in `mi_theap_malloc_zero_nonnull` by taking `ThreadHeapLocal` internally.
- [x] Fix `*mut mi_theap_t` in `_mi_theap_malloc_zero_ex` by keeping the raw ABI shim and moving typed dispatch below it.
- [x] Fix `*mut mi_theap_t` in `_mi_theap_malloc_zero` by keeping it as a raw shim over typed allocation dispatch.
- [x] Fix `*mut mi_theap_t` in `mi_theap_malloc` by keeping it as a raw shim over typed allocation dispatch.
- [x] Fix `*mut mi_theap_t` in `mi_theap_zalloc_small` by keeping it as a raw shim over typed allocation dispatch.
- [x] Fix `*mut mi_theap_t` in `mi_theap_zalloc` by keeping it as a raw shim over typed allocation dispatch.
- [x] Fix `*mut mi_theap_t` in `mi_theap_calloc` by using `CallocRequest` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_calloc_req` by taking `CallocRequest` and `ThreadHeapLocal` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_umalloc` by using typed size and `ThreadHeapLocal` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_mallocn_req` by taking `AllocProduct` and `ThreadHeapLocal` internally.
- [x] Fix `*mut mi_theap_t` in `_mi_theap_realloc_zero` by taking `ReallocRequest` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_realloc_apply` by taking `ReallocRequest`/`PreserveBytes` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_realloc` by keeping it as a raw shim over typed realloc dispatch.
- [x] Fix `*mut mi_theap_t` in `mi_theap_realloc_req` by taking `ReallocRequest` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_realloc_request` by taking `ReallocRequest` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_reallocn` by using `AllocProduct` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_reallocn_req` by taking `AllocProduct` and `ReallocRequest` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_reallocf_req` by taking `ReallocRequest` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_rezalloc` by taking typed zero-tail realloc inputs internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_recalloc` by using `CallocRequest`/`ReallocRequest` internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_recalloc_req` by taking typed request values internally.
- [x] Fix `*mut mi_theap_t` in `mi_theap_realpath` by taking `ThreadHeapLocal` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_malloc_small` by adding a typed `HeapPtr`/`BorrowedHeap` internal helper.
- [x] Fix `*mut mi_heap_t` in `mi_heap_malloc` by adding a typed `HeapPtr`/`BorrowedHeap` internal helper.
- [x] Fix `*mut mi_heap_t` in `mi_heap_zalloc_small` by adding a typed `HeapPtr`/`BorrowedHeap` internal helper.
- [x] Fix `*mut mi_heap_t` in `mi_heap_zalloc` by adding a typed `HeapPtr`/`BorrowedHeap` internal helper.
- [x] Fix `*mut mi_heap_t` in `mi_heap_calloc` by using `BorrowedHeap` plus `CallocRequest` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_mallocn` by using `BorrowedHeap` plus `AllocProduct` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_realloc` by using `BorrowedHeap` plus `ReallocRequest` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_reallocn` by using `BorrowedHeap`, `AllocProduct`, and `ReallocRequest` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_reallocf` by using `BorrowedHeap` plus `ReallocRequest` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_rezalloc` by using typed zero-tail realloc inputs internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_recalloc` by using `BorrowedHeap`, `CallocRequest`, and `ReallocRequest` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_strdup` by taking `BorrowedHeap` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_strndup` by taking `BorrowedHeap` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_realpath` by taking `BorrowedHeap` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_try_new` by taking `BorrowedHeap` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_alloc_new` by taking `BorrowedHeap` internally.
- [x] Fix `*mut mi_heap_t` in `mi_heap_alloc_new_n` by taking `BorrowedHeap` and `AllocProduct` internally.
- [x] Fix bare allocation sizes in `src/alloc.rs` by converting to `RequestedSize`/`PaddedSize` immediately after ABI parsing.
- [x] Fix bare allocation counts in `src/alloc.rs` by converting to `ItemCount`/`AllocProduct` immediately after ABI parsing.
- [x] Fix bare alignments in `src/alloc.rs` by converting to `Alignment`/`MaybeAlignment` immediately after ABI parsing.

### `src/alloc_aligned.rs`

- [x] Fix `mi_theap_malloc_guarded_aligned` by taking `ThreadHeapLocal`, `LayoutRequest`, `Alignment`, and `AlignmentOffset` internally.
- [x] Fix `mi_theap_malloc_zero_no_guarded` by taking `ThreadHeapLocal` and typed layout/alignment inputs internally.
- [x] Fix `mi_theap_malloc_zero_aligned_at_overalloc` by taking typed layout/alignment/offset inputs internally.
- [x] Fix `mi_theap_malloc_zero_aligned_at_generic` by taking typed layout/alignment/offset inputs internally.
- [x] Fix `mi_theap_malloc_zero_aligned_at` by taking typed layout/alignment/offset inputs internally.
- [x] Fix `mi_theap_malloc_aligned_at` by taking typed layout/alignment/offset inputs internally.
- [x] Fix `mi_theap_malloc_aligned` by taking typed layout/alignment inputs internally.
- [x] Fix `mi_theap_zalloc_aligned_at` by taking typed layout/alignment/offset inputs internally.
- [x] Fix `mi_theap_zalloc_aligned` by taking typed layout/alignment inputs internally.
- [x] Fix `mi_theap_calloc_aligned_at` by taking `CallocRequest`, `Alignment`, and `AlignmentOffset` internally.
- [x] Fix `mi_theap_calloc_aligned` by taking `CallocRequest` and `Alignment` internally.
- [x] Fix `mi_theap_realloc_zero_aligned_at` by taking `ReallocRequest`, `Alignment`, and `AlignmentOffset` internally.
- [x] Fix `mi_theap_realloc_zero_aligned_at_req` by taking `ReallocRequest`, `Alignment`, and `AlignmentOffset` internally.
- [x] Fix `mi_theap_realloc_zero_aligned` by taking `ReallocRequest` and `Alignment` internally.
- [x] Fix `mi_theap_realloc_aligned_at` by taking `ReallocRequest`, `Alignment`, and `AlignmentOffset` internally.
- [x] Fix `mi_theap_realloc_aligned` by taking `ReallocRequest` and `Alignment` internally.
- [x] Fix `mi_theap_rezalloc_aligned_at` by taking typed zero-tail realloc and alignment inputs internally.
- [x] Fix `mi_theap_rezalloc_aligned` by taking typed zero-tail realloc and alignment inputs internally.
- [x] Fix `mi_theap_recalloc_aligned_at` by taking `CallocRequest`, `ReallocRequest`, `Alignment`, and `AlignmentOffset` internally.
- [x] Fix `mi_theap_recalloc_aligned` by taking `CallocRequest`, `ReallocRequest`, and `Alignment` internally.
- [x] Fix `mi_heap_malloc_aligned_at` by adding a `BorrowedHeap` typed internal helper.
- [x] Fix `mi_heap_malloc_aligned` by adding a `BorrowedHeap` typed internal helper.
- [x] Fix `mi_heap_zalloc_aligned_at` by adding a `BorrowedHeap` typed internal helper.
- [x] Fix `mi_heap_zalloc_aligned` by adding a `BorrowedHeap` typed internal helper.
- [x] Fix `mi_heap_calloc_aligned_at` by adding a `BorrowedHeap` + `CallocRequest` typed helper.
- [x] Fix `mi_heap_calloc_aligned` by adding a `BorrowedHeap` + `CallocRequest` typed helper.
- [x] Fix `mi_heap_realloc_aligned_at` by adding a `BorrowedHeap` + `ReallocRequest` typed helper.
- [x] Fix `mi_heap_realloc_aligned` by adding a `BorrowedHeap` + `ReallocRequest` typed helper.
- [x] Fix `mi_heap_rezalloc_aligned_at` by adding typed zero-tail realloc helper.
- [x] Fix `mi_heap_rezalloc_aligned` by adding typed zero-tail realloc helper.
- [x] Fix `mi_heap_recalloc_aligned_at` by adding `BorrowedHeap`, `CallocRequest`, and `ReallocRequest` typed helper.
- [x] Fix `mi_heap_recalloc_aligned` by adding `BorrowedHeap`, `CallocRequest`, and `ReallocRequest` typed helper.

### `src/arena.rs`

- [x] Fix `mi_heap_visit_page` by using `PagePtr` internally.
- [x] Fix `mi_page_arena_pages` by using `PagePtr` internally.
- [x] Fix `mi_heap_has_page` by using `HeapPtr`/`BorrowedHeap` and `PagePtr` internally.
- [x] Fix `mi_page_full_size` by using `PagePtr` and typed size wrappers internally.
- [x] Fix `mi_abandoned_page_unown` by using `AbandonedPage`/`OwnedAbandonedPage` and `ThreadHeapLocal` internally.
- [x] Fix `_mi_arenas_page_free` by taking `AllFreePage<_>` and `ThreadHeapLocal` internally.
- [x] Fix `_mi_arenas_page_abandon` by taking `PageForAbandon` internally.
- [x] Fix `_mi_arenas_page_try_reabandon_to_mapped` by taking `OwnedAbandonedPage` internally.
- [x] Fix `_mi_arenas_page_unabandon` by taking `OwnedAbandonedPage` and optional `ThreadHeapLocal` internally.
- [x] Fix `mi_page_commit_usage` by taking `PagePtr` and `CommittedBytes` internally.
- [x] Fix `_mi_heap_visit_blocks` by taking `BorrowedHeap` internally.
- [x] Fix `mi_heap_visit_blocks` by keeping the raw API shim and dispatching to typed `BorrowedHeap` internals.
- [x] Fix `mi_heap_visit_abandoned_blocks` by taking `BorrowedHeap` internally.
- [x] Fix `mi_heap_delete_pages` by taking `OwnedHeap` internally.
- [x] Fix `_mi_heap_move_pages` by taking `OwnedHeap`/`BorrowedHeap` typed heap wrappers internally.
- [x] Fix `_mi_heap_destroy_pages` by taking `OwnedHeap` internally.
- [x] Fix `mi_heap_arena_pages` by taking `BorrowedHeap` and `ArenaPtr` internally.
- [x] Fix `mi_arena_start_idx` by taking `BorrowedHeap`/`ArenaPtr` and `ArenaIndex` internally.
- [x] Fix `mi_arenas_try_find_free` by taking typed heap/subproc/arena indexes and slice counts internally.
- [x] Fix `mi_arenas_try_alloc` by taking `BorrowedHeap`, `SliceCount`, `Alignment`, and `NumaNode` internally.
- [x] Fix `_mi_arenas_alloc_aligned` by taking typed size/alignment/slice domains internally.
- [x] Fix `_mi_arenas_alloc` by taking typed size/slice domains internally.
- [x] Fix `mi_heap_ensure_arena_pages` by taking `BorrowedHeap` and typed arena/page-table domains internally.
- [x] Fix `mi_debug_show_arenas_ex` by taking `BorrowedHeap` or typed nullable heap wrapper internally.
- [x] Fix `mi_arena_id_from_arena` by taking `ArenaPtr` internally.
- [x] Fix `mi_arena_is_suitable` by taking `ArenaPtr`, `NumaNode`, and typed flags internally.
- [x] Fix `_mi_arena_memid_is_suitable` by taking `MemIdView`/typed memid plus typed arena input internally.
- [x] Fix `mi_arena_info_slices` by taking `ArenaPtr` and `SliceRange` internally.
- [x] Fix `mi_arena_commit` by taking `ArenaPtr`, `SliceRange`, and `CommittedBytes` internally.
- [x] Fix `mi_arena_size` by taking `ArenaPtr` and returning `ReservedBytes` internally.
- [x] Fix `mi_arena_start` by taking `ArenaPtr` and returning `SliceStart`/`OsMemPtr` internally.
- [x] Fix `mi_arena_slice_start` by taking `ArenaPtr` and `SliceIndex` internally.
- [x] Fix `mi_arena_page_at_slice` by taking `ArenaPtr` and `SliceIndex` internally.
- [x] Fix `mi_arena_strictly_contains` by taking `ArenaPtr` and typed pointer/range inputs internally.
- [x] Fix `mi_arenas_contain_ex` by taking typed pointer/range inputs internally.
- [x] Fix `mi_arena_pages_alloc` by taking `ArenaPtr`, `SliceRange`, and typed size domains internally.
- [x] Fix `mi_arenas_add` by taking `SubprocPtr`, `ArenaPtr`, and `ArenaIndex` internally.
- [x] Fix `mi_arena_initialize` by taking `SubprocPtr`, `OsMemPtr`, `SliceCount`, and `NumaNode` internally.
- [x] Fix `mi_arena_schedule_purge` by taking `ArenaPtr` and `SliceRange` internally.
- [x] Fix `mi_arena_purge` by taking `ArenaPtr` and `SliceRange` internally.
- [x] Fix `mi_arena_try_purge_range` by taking `ArenaPtr` and `SliceRange` internally.
- [x] Fix `mi_arena_try_purge_visitor` by taking typed arena/range visitor inputs internally.
- [x] Fix `mi_arenas_get_count` by taking `SubprocPtr` internally.
- [x] Fix `mi_arena_from_index` by taking `SubprocPtr` and `ArenaIndex` internally.
- [x] Fix `mi_arenas_unsafe_destroy` by taking `SubprocPtr` internally.
- [x] Fix `_mi_arenas_unsafe_destroy_all` by taking typed subproc iteration inputs internally.
- [x] Fix `mi_arena_reserve` by taking `SubprocPtr`, `ReservedBytes`, and `NumaNode` internally.
- [x] Fix `mi_arenas_try_purge` by taking `SubprocPtr` and typed purge inputs internally.
- [x] Fix `mi_reserve_os_memory_ex2` by taking `SubprocPtr`, `ReservedBytes`, and typed flags internally.
- [x] Fix `mi_manage_os_memory_ex2` by taking `SubprocPtr`, `OsMemPtr`, `ReservedBytes`, and typed flags internally.

### `src/free.rs`

- [x] Fix `mi_block_unguard` by taking typed page, guarded block, and user-pointer witnesses internally.
- [x] Fix `mi_block_check_unguard` by taking typed page/block/user witnesses internally.
- [x] Fix `mi_block_for_free_from_lookup` by moving construction into typed validation and removing raw page/user pointer parameters.
- [x] Fix `mi_free_generic_local` by making it a raw dispatch boundary that immediately produces `ValidatedFree` and `LocalPage`.
- [x] Fix `mi_free_generic_mt` by making it a raw dispatch boundary that immediately produces `ValidatedFree` and `ForeignPage`.
- [x] Fix `_mi_free_generic` by keeping it as a raw compatibility dispatcher only.
- [x] Fix `mi_abandoned_page_try_free` by taking `AbandonedPage`/`AllFreePage` internally.
- [x] Fix `mi_abandoned_page_try_reabandon_to_mapped` by taking `OwnedAbandonedPage` internally.
- [x] Fix `mi_abandoned_page_unown_from_free` by taking `OwnedAbandonedPage` and `RemoteFreeList`/`ThreadFreeWord` internally.
- [x] Fix `mi_page_queue_len_is_atmost` by taking `ThreadHeapLocal`, `BlockSize`, and `PageQueueCount`/typed limit values.
- [x] Fix `mi_abandoned_page_try_reclaim` by taking `OwnedAbandonedPage` and typed reclaim limits internally.
- [x] Fix `mi_free_try_collect_mt` by taking `OwnedAbandonedPage` and typed remote-free head internally.

### `src/internal.rs`

- [x] Fix `mi_page_set_has_interior_pointers` by taking `PagePtr` and using `AtomicPageThreadId`.
- [x] Fix `mi_page_set_abandoned_mapped` by taking `PagePtr` and using `AtomicPageThreadId`.
- [x] Fix `mi_page_clear_abandoned_mapped` by taking `PagePtr` and using `AtomicPageThreadId`.
- [x] Fix `mi_page_claim_ownership` by taking `AbandonedPage` and returning `Option<OwnedAbandonedPage>`.
- [x] Fix `mi_page_set_keys` by taking `PagePtr` and `PageKeys`.
- [x] Fix `mi_tf_block` by converting it to a `ThreadFreeWord` compatibility shim or deleting it after callers migrate.
- [x] Fix `mi_tf_is_owned` by converting it to a `ThreadFreeWord` compatibility shim or deleting it after callers migrate.
- [x] Fix `mi_tf_create` by converting it to a `ThreadFreeWord` compatibility shim or deleting it after callers migrate.
- [x] Fix `mi_block_set_nextx` by taking `BlockPtr`, `PagePtr`, and `EncodedPtr`/`BlockLink` internally.
- [x] Fix `mi_block_set_next` by taking `BlockPtr`, `PagePtr`, and `EncodedPtr`/`BlockLink` internally.
- [x] Fix `_mi_malloc_generic` by taking `ThreadHeapPtr`, `LayoutRequest`, and `MallocGenericFlags` internally.
- [x] Fix `_mi_theap_get_free_small_page_typed` by taking `ThreadHeapLocal` and `PaddedSize`.
- [x] Fix `mi_theap_malloc_use_guarded` by taking `ThreadHeapLocal` and `RequestedSize`/`EffectiveSize`.

### `src/page.rs`

- [x] Fix `mi_page_list_count` by using `PagePtr` internally.
- [x] Fix `mi_page_list_is_valid` by using `PagePtr` internally.
- [x] Fix `mi_page_is_valid_init` by using `PagePtr`/`PageConstPtr` internally.
- [x] Fix `_mi_page_is_valid` by using `PagePtr`/`PageConstPtr` internally.
- [x] Fix `mi_page_thread_collect_to_local` by using `OwnedAbandonedPage`, `RemoteFreeList`, and typed free-list wrappers.
- [x] Fix `mi_page_thread_free_collect` by using `LocalPage`/`OwnedAbandonedPage` and `AtomicThreadFree` internally.
- [x] Fix `mi_page_free_quick_collect` by using `LocalPage` and typed free-list wrappers internally.
- [x] Fix `_mi_page_free_collect` by using typed page ownership witnesses and free-list wrappers internally.
- [x] Fix `_mi_page_free_collect_partly` by using typed remote-free head and page witnesses internally.
- [x] Fix `_mi_page_unfull` by making it a raw shim over `FullPage::unfull`.
- [x] Fix `mi_page_to_full` by making it a raw shim over `ActivePage::to_full`.
- [x] Fix `_mi_page_free` by making it a raw shim over `AllFreePage<_>::free_to_arena`.
- [x] Fix `_mi_page_retire` by making it a raw shim over typed retire transitions.
- [x] Fix `mi_page_free_list_extend_secure` by taking typed page/block/size domains internally.
- [x] Fix `mi_page_free_list_extend` by taking typed page/block/size domains internally.
- [x] Fix `mi_page_extend_free` by taking typed page/block/size domains internally.
- [x] Fix `_mi_page_abandon` by making it a raw shim over typed abandonment transitions.
- [x] Fix `_mi_page_init` by using `Page<Uninit>`, `ThreadHeapLocal`, and `ActivePage` transitions internally.
- [x] Fix `mi_page_fresh_alloc` by using `ThreadHeapLocal`, `LayoutRequest`, and typed page domains internally.
- [x] Fix `mi_page_fresh` by using `ThreadHeapLocal`, `SliceCount`, and `ActivePage` internally.
- [x] Fix `mi_theap_page_reclaim_raw` by keeping it as a raw shim over typed reclaim.
- [x] Fix `mi_page_flags_set` by using `PageFlags` and `AtomicPageThreadId`.
- [x] Fix `mi_page_set_in_full` by using `PageFlags` and `AtomicPageThreadId`.
- [x] Fix `mi_page_set_theap` by using `ThreadHeapLocal`/`PageOwnerId` and `AtomicPageThreadId`.
- [x] Fix `mi_page_queue_remove` by using typed queue/page witnesses internally.
- [x] Fix `mi_page_queue_push` by using typed queue/page witnesses internally.
- [x] Fix `mi_page_queue_push_at_end` by using typed queue/page witnesses internally.
- [x] Fix `mi_page_queue_move_to_front` by using typed queue/page witnesses internally.
- [x] Fix `_mi_theap_collect_retired` by taking `ThreadHeapLocal` internally.
- [x] Fix `_mi_page_queue_is_valid` by taking typed queue wrappers internally.
- [x] Fix `mi_theap_page_queue_of` by taking `ThreadHeapLocal` and `BlockSize` internally.
- [x] Fix `mi_theap_queue_first_update` by taking `ThreadHeapLocal` and typed queue wrappers internally.
- [x] Fix `_mi_page_queue_append` by taking typed queue wrappers internally.
- [x] Fix `mi_page_queue_find_free_ex` by taking `ThreadHeapLocal`, `BlockSize`, and typed search limits internally.
- [x] Fix `mi_find_free_page` by taking `ThreadHeapLocal` and typed queue wrappers internally.
- [x] Fix `_mi_deferred_free` by taking `ThreadHeapLocal`, `Heartbeat`, and typed callback wrappers internally.
- [x] Fix `mi_huge_page_alloc` by taking `ThreadHeapLocal`, `LayoutRequest`, and typed huge-alignment inputs internally.
- [x] Fix `mi_find_page` by taking `ThreadHeapLocal`, `PaddedSize`, and typed huge-alignment inputs internally.
- [x] Fix `_mi_malloc_generic` by keeping it as a raw shim over `_mi_malloc_generic_typed`.
- [x] Fix `_mi_malloc_generic_typed` by taking `ThreadHeapLocal`/`ThreadHeapPtr`, `LayoutRequest`, and `MallocGenericFlags` internally.

### `src/page_map.rs`

- [x] Fix duplicate `mi_page_map_get_idx` definitions by consolidating them into one typed implementation and raw wrappers.
- [x] Fix `_mi_page_map_register` by taking `PagePtr` and `SliceCount` below the ABI boundary and returning/recording `RegisteredPageRange`.
- [x] Fix `_mi_page_map_unregister` by taking `PagePtr`, `SliceCount`, and `RegisteredPageRange` below the ABI boundary.
- [x] Fix `mi_page_map_set_range_prim` by taking `PagePtr`, `PageMapIndex`, `PageMapSubIndex`, and `SliceCount` internally.
- [x] Fix `mi_page_map_set_range` by taking `PagePtr`, `PageMapIndex`, `PageMapSubIndex`, and `SliceCount` internally.
- [x] Fix `mi_page_map_ensure_committed` by taking `PageMapIndex` and `SliceCount` internally.

### `src/prim.rs`

- [x] Fix `_mi_page_associated_theap_peek` by taking `PagePtr` and returning `Option<ThreadHeapPtr>`.
- [x] Fix `_mi_tls_theap_default_set` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.
- [x] Fix `_mi_tls_theap_cached_set` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.
- [x] Fix `mi_linux_x64_theap_default_set` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.
- [x] Fix `mi_linux_x64_theap_cached_set` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.
- [x] Fix `mi_prim_thread_local_default_set` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.
- [x] Fix `mi_prim_thread_local_cached_set` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.

### `src/stats.rs`

- [x] Fix `mi_heap_get_stats` by taking `HeapPtr`/`BorrowedHeap` internally.
- [x] Fix `mi_heap_stats_merge_to_subproc` by taking `BorrowedHeap` and `SubprocPtr` internally.
- [x] Fix `mi_heap_stats_print_out` by taking `BorrowedHeap` and typed callback/output wrappers internally.
- [x] Fix `mi_heap_print_visitor` by taking typed visitor state internally.
- [x] Fix `mi_heap_aggregate_visitor` by taking typed visitor state internally.
- [x] Fix `mi_heap_stats_get` by taking `BorrowedHeap` internally.
- [x] Fix `mi_heap_stats_get_json` by taking `BorrowedHeap` and typed buffer domains internally.
- [x] Fix `mi_stats_merge_theap_to_heap` by taking `ThreadHeapPtr`/`ThreadHeapLocal` and `HeapPtr` internally.
- [x] Fix stats bin and counter indexes by using `StatsBin`, `Bin`, and typed count/byte wrappers.

### `src/theap.rs`

- [x] Fix `_mi_heap_area_init` by taking `HeapPtr`, `PagePtr`, and typed area fields internally.
- [x] Fix `_mi_theap_area_visit_blocks` by taking `ThreadHeapPtr`, `PagePtr`, and typed visitor state internally.
- [x] Fix `mi_theap_visit_areas_page` by taking `ThreadHeapPtr`, `PagePtr`, and typed visitor state internally.
- [x] Fix `mi_theap_visit_pages` by taking `ThreadHeapPtr` and typed visitor state internally.
- [x] Fix `mi_theap_page_is_valid` by taking `ThreadHeapPtr` and `PagePtr` internally.
- [x] Fix `mi_theap_page_collect` by taking `ThreadHeapLocal` and `PagePtr`/page-state witnesses internally.
- [x] Fix `mi_theap_is_valid` by taking `ThreadHeapPtr` internally.
- [x] Fix `mi_theap_merge_stats` by taking `ThreadHeapPtr` internally.
- [x] Fix `mi_theap_collect_ex` by taking `ThreadHeapLocal` and typed collect mode internally.
- [x] Fix `_mi_theap_collect_abandon` by taking `ThreadHeapLocal` internally.
- [x] Fix `mi_theap_collect` by keeping it as a raw shim over typed collect internals.
- [x] Fix `mi_theap_set_default` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.
- [x] Fix `_mi_theap_init` by taking typed heap/thread-heap/tld wrappers internally.
- [x] Fix `_mi_theap_random_next` by taking `ThreadHeapLocal` internally.
- [x] Fix `mi_theap_free_mem` by taking `ThreadHeapLocal` and typed memid/range inputs internally.
- [x] Fix `_mi_theap_incref` by using typed refcount wrappers internally.
- [x] Fix `mi_theap_decref_inline` by using typed refcount wrappers internally.
- [x] Fix `_mi_theap_decref` by using typed refcount wrappers internally.
- [x] Fix `_mi_theap_free` by taking typed thread-heap state wrappers internally.
- [x] Fix `_mi_theap_delete` by taking typed thread-heap state wrappers internally.
- [x] Fix `mi_heap_collect` by keeping it as a raw heap shim over typed heap collect internals.
- [x] Fix `_mi_theap_create` by returning `ThreadHeapPtr` or `ThreadHeapLocal` below the raw heap shim.

### `src/heap.rs`

- [x] Fix `mi_heap_theap` by using `HeapPtr`/`ThreadHeapPtr` internally.
- [x] Fix `mi_heap_set_numa_affinity` by using `HeapPtr` and `NumaNode` internally.
- [x] Fix `mi_heap_stats_merge_to_main` by using `BorrowedHeap` internally.
- [x] Fix `mi_heap_free_theaps` by using `OwnedHeap` internally.
- [x] Fix `mi_heap_free` by using `OwnedHeap` internally.
- [x] Fix `mi_heap_delete` by keeping it as a raw shim over typed owned-heap deletion.
- [x] Fix `_mi_heap_force_destroy` by using `OwnedHeap` internally.
- [x] Fix `mi_heap_destroy` by keeping it raw/unsafe and preventing accidental use through safe or typed owned-heap delete paths.
- [x] Fix `mi_heap_check_owned` by using `BorrowedHeap`/`OwnedHeap` typed ownership checks.
- [x] Fix `mi_unsafe_heap_page_is_under_utilized` by using `BorrowedHeap` and `PagePtr` internally.

### `src/init.rs`

- [x] Fix `_mi_tls_theap_main_set` by taking `ThreadHeapPtr` internally.
- [x] Fix `mi_linux_x64_theap_main_set` by taking `ThreadHeapPtr` internally.
- [x] Fix `mi_theap_main_tls_set` by taking `ThreadHeapPtr` internally.
- [x] Fix `_mi_thread_done` by using typed thread-heap state wrappers internally.
- [x] Fix `mi_theap_guarded_set_sample_rate` by taking `ThreadHeapLocal` and typed guarded config fields internally.
- [x] Fix `mi_theap_guarded_set_size_bound` by taking `ThreadHeapLocal` and typed guarded config fields internally.
- [x] Fix `_mi_theap_guarded_init` by taking `ThreadHeapLocal` and `GuardedConfig` internally.
- [x] Fix `_mi_theap_options_init` by taking `ThreadHeapLocal` internally.
- [x] Fix `_mi_theap_default_set` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.
- [x] Fix `_mi_theap_cached_set` by taking `ThreadHeapPtr`/`ThreadHeapLocal` internally.
- [x] Fix `mi_heap_destroy` import/use by routing through typed owned-heap deletion where applicable.
- [x] Fix `mi_subproc_prefix` by taking `SubprocPtr` internally.
- [x] Fix `mi_subproc_heap_main_raw` by returning `MainHeap`/`HeapPtr` internally.
- [x] Fix `_mi_subproc_to_id` by taking `SubprocPtr` internally.
- [x] Fix `mi_subproc_destroy_meta` by taking `SubprocPtr` internally.
- [x] Fix `mi_subproc_unsafe_destroy` by taking `SubprocPtr` internally.
- [x] Fix `mi_tld_alloc_non_main` by taking `SubprocPtr` and typed TLD wrappers internally.
- [x] Fix `_mi_subproc_heap_main` by returning `MainHeap`/`HeapPtr` internally.

### `src/os.rs`

- [x] Fix `mi_os_prim_free` by taking typed memid/subproc inputs internally.
- [x] Fix `_mi_os_free_ex` by taking typed memid, pointer, size, and subproc inputs internally.
- [x] Fix `mi_os_free_huge_os_pages` by taking typed pointer/size/subproc inputs internally.

### `src/bitmap.rs` and `src/threadlocal.rs`

- [x] Fix `mi_bitmap_try_find_and_claim` by taking typed arena/bitmap index/range wrappers where the operation is arena-specific.
- [x] Fix `_mi_bitmap_forall_set` by taking typed bitmap index/range wrappers where applicable.
- [x] Fix `_mi_bitmap_forall_setc_ranges` by taking typed bitmap range wrappers where applicable.
- [x] Fix `_mi_bitmap_forall_setc_rangesn` by taking typed bitmap range/count wrappers where applicable.
- [x] Fix `mi_thread_local_claim_fun` by taking `SubprocPtr` or typed callback wrappers internally.

---

## Phase 17: Raw Boundary And Public API Hygiene

- [x] Keep `src/types.rs` as the private raw layout substrate for translated
  `#[repr(C)]` layouts and aliases; do not replace C structs with strict wrappers.
- [x] Keep public C layout exposure routed through `raw::abi`/public raw facade
  modules, not through `core_types`.
- [x] Keep `src/abi.rs` C declarations raw; strict types must not leak into C ABI
  declarations.
- [x] Keep `src/raw.rs` as the explicit C-shaped facade; strict types remain
  crate-private.
- [x] Ensure `core_types` is not exported at crate root or through default
  `raw` modules.
- [x] Ensure `unstable-raw-internals` remains the only way tests can inspect
  translated internals through `raw`.
- [x] Update `docs/api-inventory.md` if any public exposure changes are required
  by the migration.
- [x] Update `docs/abi.md` with the final strict-type/raw-boundary split.
- [x] Update `docs/compatibility.md` to classify `core_types` as private design
  surface, not public SemVer.

---

## Phase 18: Zero-Cost Proof And Performance Signoff

- [x] Add a release assembly capture script for `mi_malloc`, `mi_malloc_small`,
  `mi_free`, and `_mi_theap_malloc_zero_ex`.
- [x] Add an assembly comparison checklist proving strict wrappers add no extra
  branches or memory traffic in the four required hot paths.
- [x] Capture assembly for `_mi_page_malloc_zero_typed` free-list pop before and
  after final free-list wrapper migration.
- [x] Capture assembly for local free fast path before and after final
  `ValidatedFree`/`LocalPage` witness hardening.
- [x] Capture assembly for remote free CAS loop before and after final
  `AtomicThreadFree` migration.
- [x] Capture assembly for small allocation page lookup after `PaddedSize` and
  `DirectBin` migration.
- [x] Run benchmark smoke checks after each hot-path migration chunk.
- [x] Run targeted Criterion/native allocator benchmark subset after final
  hot-path migration.
- [x] Record benchmark decision and assembly evidence in `BENCHMARK_RESULTS.md`
  or a linked strict-type performance report.
- [x] If a wrapper adds measurable hot-path cost, either redesign it to compile
  away or document why the specific invariant is kept only in cold code.

---

## Phase 19: Final Strict-Type Signoff

- [x] Remove or close every strict-type allowlist entry that is not a true raw
  C/compat boundary.
- [x] Confirm no non-ABI internal function accepts raw page/block/heap/theap/
  arena/subproc pointers where a wrapper exists.
- [x] Confirm no non-ABI internal function accepts bare `usize` for a domain that
  has a wrapper.
- [x] Confirm `mi_free_block_local` accepts only `LocalPage` and unforgeable
  `BlockForFree`.
- [x] Confirm remote free accepts only `ForeignPage` or abandoned-page witnesses.
- [x] Confirm reclaim accepts only `OwnedAbandonedPage` and `ThreadHeapLocal`.
- [x] Confirm full-queue operations accept only `FullPage` and `FullPageQueue`.
- [x] Confirm normal queue operations accept only active non-full pages and
  matching `NormalPageQueue`.
- [x] Confirm memid union reads occur only through `MemId::view` or typed
  constructors and handle unknown/corrupt values without UB.
- [x] Confirm `xthread_id` bit operations occur only through `PageThreadId`,
  `PageFlags`, or `AtomicPageThreadId`.
- [x] Confirm `xthread_free` bit operations occur only through `ThreadFreeWord`
  or `AtomicThreadFree`.
- [x] Confirm `mi_block_s.next` is read/written only through `BlockLink`,
  `EncodedPtr`, or free-list wrappers.
- [x] Confirm pointer-to-atomic casts occur only through typed atomic field
  wrappers.
- [x] Confirm C-compatible `mi_memory_order_e` is confined to raw/atomic
  compatibility code and internals use `MemoryOrder`/`Ordering`.
- [x] Confirm callback function/context pointers are typed internally and raw
  callback typedefs remain only at C ABI boundaries.
- [x] Confirm optimized allocator internals remain `no_std`/no-allocation and do
  not introduce heap-allocating Rust library types.
- [x] Confirm pointer-to-integer round trips occur only through typed packed
  pointer/provenance abstractions.
- [x] Confirm broad Rust references to aliasable allocator metadata are not used;
  metadata access goes through pointer wrapper field helpers.
- [x] Confirm pointer wrappers do not implement `Deref` or `DerefMut`.
- [x] Confirm all hot wrappers are transparent, copyable where applicable, and
  inlineable.
- [x] Confirm layout tests pass across `secure`, `guarded`, `track-valgrind`, and
  `track-asan` feature combinations.
- [x] Confirm all strict-type docs are up to date and no docs claim the migration
  is merely staged.
- [x] Run the full validation set and record final strict-type completion notes.
