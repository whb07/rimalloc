# Strict Type Architecture

`TYPES_SPEC.md` is the acceptance source for the allocator's internal type
model. `MIGRATION_PHASES.md` records the implementation status. This document
is the short maintainer map for the current `src/types/` modules.

The strict-type layer is crate-private. It exists to make allocator invariants
explicit without changing the raw C ABI or hot-path code shape. Raw C-compatible
types remain in `src/types/` and C-shaped declarations remain behind
`mechanical_mimalloc::raw`.

## Status Labels

- `live`: used by current allocator paths.
- `live scaffold`: used by current paths and still contains checked helper
  wrappers for adjacent invariants that are covered by source audits.
- `compat boundary`: exists to bridge typed internals with translated C-shaped
  code.

## Module Map

| Module | Status | Invariant |
|---|---|---|
| `scalar` | live scaffold | Separates requested, effective, padded, usable, block, bin, slice, and alignment domains. |
| `ptr` | live scaffold | Provides non-null transparent metadata/data pointer wrappers without `Deref` or `DerefMut`. |
| `layout` | live | Centralizes malloc, calloc, realloc, zeroing, padding, alignment, and huge-alignment request validation. |
| `page` | live | Splits tagged page owner bits, page flags, and local/remote fast-path classification. |
| `freelist` | live | Packs and unpacks thread-free list owner/head words. |
| `freelist_wrappers` | live | Owns encoded block links, guarded tags, and available/local/remote free-list movement. |
| `validation` | live scaffold | Models pointer lookup results, guarded/interior locations, and validated free requests. |
| `validation_extra` | live | Carries allocated/free block witnesses used by free-list and allocation paths. |
| `page_states` | live scaffold | Provides page typestate witnesses for local, foreign, full, abandoned, and all-free paths. |
| `page_queues` | live | Requires typed queue/page witnesses for full/unfull transitions. |
| `memid` | live scaffold | Restricts union-arm interpretation to typed `MemId::view` and constructors. |
| `atomic` | live | Centralizes pointer-to-atomic field casts and fixed acquire/release/acqrel metadata operations. |
| `padding` | live | Owns padding pointer, canary, and delta access for allocation/free support paths. |
| `arena` | live scaffold | Models arena slice ranges and bounds-checked slice counts. |
| `heap` | live | Provides heap/thread-heap/subprocess ownership and borrow witnesses used below raw shims. |
| `free_actions` | live | Carries local free mode options as an explicit value. |

## Page Owner And Free-Path Signoff

`TYPES_SPEC.md` sections 6.2 and 6.3 are live in
`src/types/page.rs`.

- `ThreadId::new` rejects zero and ids with low page-flag bits set;
  `new_unchecked` remains for proven raw thread-id reads.
- `PageFlags::new` rejects bits outside `MI_PAGE_FLAG_MASK`;
  `from_low_bits` remains the extractor for already-packed raw words.
- `PageThreadId::{owner, flags, is_abandoned, is_abandoned_mapped,
  with_flags, with_owner, from_parts}` are the page owner/flag packing API.
- `PageThreadId::xor_current`, `PageThreadId::classify_free_path`, and
  `XThreadDelta::{is_local_fast, needs_local_slow, is_remote_plain}` preserve
  the XOR-based free-path classification shape.

## Dead-Code Discipline

`types` does not rely on broad dead-code allowances. Retained proof and
compatibility helpers must either be used by allocator code, removed, or tied to
narrow compile-time reachability references that name the exact function,
method, enum payload, or field being preserved.

Do not add a broad module-level `#![allow(dead_code)]` to `types`.

## Verification

The release fast-path audit checks the current source-shape contract:

- `types` owns the semantic wrapper modules,
- metadata wrappers do not implement `Deref` or `DerefMut`,
- memid union reads stay behind `MemId::view` or typed constructors,
- pointer-to-atomic casts stay behind `Atomic*Field` wrappers,
- block `next` access stays behind free-list wrappers,
- padding metadata access stays behind `PaddingPtr`,
- typed free/reclaim/page-queue signatures do not regress to raw page states.

Run:

```sh
cargo test --quiet --test release_fast_path_audit --features std
cargo test --quiet --lib
```

Use `scripts/check-layout.sh` when ABI/layout changes touch the strict-type
boundary.

## Final Strict-Type Signoff

- Raw `mi_page_t`, `mi_block_t`, `mi_heap_t`, `mi_theap_t`, `mi_arena_t`, and
  `mi_subproc_t` parameters are confined to raw ABI files or to active
  `strict-type-allowlist.txt` entries whose reasons identify translated C,
  platform, raw compatibility, constructor, or proof-helper boundaries. The
  current strict-type checker validates this inventory with
  `scripts/check-strict-types.py`.
- Bare-domain `usize` parameters with allocation-size, alignment, count, index,
  bin, capacity, reserved/used, slice-count, or block/chunk naming are likewise
  raw ABI boundaries or explicit allowlist entries. The allowlist now requires a
  `permanent:` or `remove:` disposition so permanent C compatibility and
  remaining migration debt are visible separately.

## Strict-Type Allowlist Governance

The current `strict-type-allowlist.txt` snapshot has 632 active entries: 330 raw-pointer-parameter,
220 bare-domain-usize, 40 metadata-reference, 23 pointer-usize-roundtrip, 14
atomic-cast, and 5 heap-allocating-type entries. It has zero wildcard entries
and zero removable entries; all current entries are classified as `permanent:`
compatibility or proof boundaries.

Permanent hotspot families are the C-shaped raw pointer domains that preserve
upstream allocator call shapes before typed helpers take over: `mi_theap_t` has
109 entries, `mi_page_t` has 110 entries, and `mi_heap_t` has 67 entries. The
largest file hotspots are `src/allocator/aligned.rs`, `src/internal/mod.rs`,
`src/allocator/alloc.rs`, `src/allocator/page.rs`, `src/internal/bitmap.rs`,
and `src/raw/macros.rs`; these are guarded by the release fast-path audit so new
growth must be intentional.

Permanent reason-group caps are also guarded by the release fast-path audit.
No permanent reason group exceeds 40 entries:
`internal page metadata helper boundary` (33) is a permanent translated-C page metadata edge,
`compatibility boundary retained for C ABI parity or translated-C source shape` (33) is a permanent raw ABI/source-shape edge,
`C/platform ABI forwarding boundary` (32) is a permanent platform ABI forwarding edge,
`aligned allocation compatibility boundary` (30) is a permanent aligned allocation ABI/source-shape edge,
`thread-heap compatibility boundary` (24) is a permanent thread-heap source-shape edge,
and `scalar domain constructor boundary` (24) is a permanent scalar constructor/proof edge.

Raw pointer file/domain caps are guarded separately:
`src/allocator/alloc.rs`/`mi_theap_t` at 27,
`src/internal/mod.rs`/`mi_page_t` at 25,
`src/allocator/page.rs`/`mi_page_t` at 21,
`src/allocator/thread_heap.rs`/`mi_theap_t` at 20,
`src/allocator/free.rs`/`mi_page_t` at 19,
and `src/allocator/page.rs`/`mi_theap_t` at 19.

Future rewrite candidates must be recorded as `remove:` entries with a concrete
follow-up task before they are allowed to grow. Likely candidates are narrow
bare-domain `usize` arithmetic around bitmap ranges, raw macro size handling,
and scalar helper boundaries where a typed wrapper can replace local translated
C arithmetic without changing public ABI or allocation hot-path behavior.
- Internal scalar domains with wrappers use typed values after raw ABI
  validation: allocation sizes/layouts (`RequestedSize`, `PaddedSize`,
  `UsableSize`, `BlockSize`), bin indexes (`Bin`, `DirectBin`), arena/page-map
  slice domains (`SliceCount`, `SliceIndex`, `SliceRange`, `PageMapIndex`,
  `PageMapSubIndex`), alignments (`Alignment`, `MaybeAlignment`,
  `AlignmentOffset`), and typed counts where wrappers exist. Remaining bare
  `usize` values are raw ABI parameters, primitive byte counts, public
  C-compatible return values, or local arithmetic after wrapper extraction.
- The local free sink `mi_free_block_local` accepts only a `LocalPage` plus
  `BlockForFree`. `BlockForFree::new` is private to `validation.rs`, and the
  only public crate path to one is `PointerLookup::prepare_free` producing
  `ValidatedFree::block()` after page/user/block validation.
- Remote free enters `mi_free_block_mt` only with a `ForeignPage` plus
  `BlockForFree`. Abandoned and abandoned-mapped page owners are converted
  through their `remote_free_page` witness methods, and remote collection
  continues through `OwnedAbandonedPage` plus `RemoteFreeList` after the raw
  compatibility shim.
- Reclaim below the raw keep shim accepts only `OwnedAbandonedPage` and
  `ThreadHeapLocal`: `try_reclaim_abandoned`, `_mi_theap_page_reclaim`, and
  `OwnedAbandonedPage::reclaim(ThreadHeapLocal)` preserve that ownership pair
  until the page is returned to the active queue.
- Full-queue operations use `FullPage`/`FullPageQueue` witnesses below raw
  shims: full append takes typed full queues, full-page freeing rebuilds a
  `QueuedPage<FullQueue>` from `FullPage` plus `FullPageQueue`, and unfull
  consumes `QueuedPage<FullQueue>` before returning an active page.
- Normal queue operations use `ActivePage` plus `NormalPageQueue` below raw
  queue helpers. `QueuedPage<NormalQueue>::new` rejects full pages and requires
  the page block size to match the queue block size before push, move-to-front,
  active-page free, or normal append paths continue.
- Memid union-arm reads are centralized in `MemId::view` or typed construction
  paths. Unknown memkind values, null meta/arena union pointers, and invalid
  arena slice ranges are represented as `MemIdView::Unknown` instead of being
  dereferenced.
- `xthread_id` owner and flag bit operations are centralized in
  `PageThreadId`, `PageFlags`, and `AtomicPageThreadId`. Raw page accessors load
  through the atomic wrapper, flag mutation validates `PageFlags`, and
  owner transitions preserve flags with typed `PageOwnerId` values.
- `xthread_free` head/ownership bit operations are centralized in
  `ThreadFreeWord` and `AtomicThreadFree`. Raw compatibility helpers decode or
  create thread-free words through the wrapper, and remote push, collect, claim,
  and release protocols use typed atomic methods.
- `mi_block_s.next` reads and writes are centralized in `EncodedPtr`,
  `BlockLink`, `BlockPtr`, and the available/local free-list wrappers. Direct
  raw field access is limited to the wrapper implementation and the C-compatible
  layout definition, with `scripts/check-strict-types.py` enforcing that
  boundary.
- Pointer-to-atomic casts for allocator metadata are centralized in
  `AtomicUsizeField`, `AtomicI64Field`, and `AtomicPtrField`.
  `src/internal/atomic.rs` remains the raw `mi_atomic_*` compatibility layer,
  while new internal metadata access goes through typed atomic field wrappers
  and the strict checker rejects direct casts elsewhere.
- C-compatible `mi_memory_order_e` values are confined to `src/internal/atomic.rs`,
  where the raw `mi_atomic_*` facade maps them to Rust `Ordering`. Internal
  allocator code uses `types::MemoryOrder` or `core::sync::atomic::Ordering`
  directly instead of propagating the C enum.
- Pointer-to-integer round trips are centralized in typed packed-pointer and
  provenance helpers: `ThreadFreeWord` owns `xthread_free` pointer/owner
  packing, `EncodedWord`/`EncodedPtr` own free-list pointer words, and
  `ptr_exposed_addr` plus the alignment/difference helpers own exposed-address
  arithmetic. Remaining translated C or platform boundaries are explicit
  `pointer-usize-roundtrip` allowlist entries checked by
  `scripts/check-strict-types.py`.
- Broad Rust references to aliasable allocator metadata are rejected by the
  strict-type checker. Metadata reads and writes go through pointer wrapper
  field helpers that use `addr_of!`, `addr_of_mut!`, `read`, and `write`;
  remaining translated C or platform raw-reference boundaries are explicit
  `metadata-reference` allowlist entries.
- Pointer wrappers and allocator typestate wrappers remain raw-pointer style
  value types. The release fast-path audit rejects `Deref` and `DerefMut`
  implementations under `src/types/`, so callers must use explicit field,
  conversion, or witness methods instead of implicit references.
- Hot strict-type wrappers stay zero-cost: pointer wrappers, copied page/queue
  witnesses, free-list wrappers, packed words, and atomic field wrappers are
  `#[repr(transparent)]`, derive `Copy` where duplication does not weaken the
  witness, expose inline constructors/accessors, and are covered by
  size/alignment plus `Option<Wrapper>` niche tests.
- Raw layout compatibility is checked by the core layout tests and release
  fast-path audit in the default build and with executable feature sets
  `secure`, `guarded`, and `track-valgrind`. `track-asan` is checked by
  compilation plus public symbol export validation because its executable tests
  require an ASan runtime setup.
- Callback function pointers and context pointers are wrapped internally by
  `DeferredFreeCallback`, `OutputCallback`, `ErrorCallback`,
  `HeapVisitCallback`, `BlockVisitCallback`, `ThreadLocalClaimCallback`, and
  their `*CallbackArg` context wrappers. Raw callback typedefs remain in
  `src/types/` and `src/ffi/abi.rs` C ABI declarations.
- Optimized allocator internals remain covered by `no_std`/no-allocation audit
  tests: strict-type modules and allocation/free hot-path helpers reject `std`,
  `alloc`, formatting allocation, synchronization primitives, and Rust
  heap-allocating library types. The strict-type source check also rejects
  dynamic dispatch in allocator, internal, platform, and strict-type modules
  unless an explicit allowlist entry records measured justification.
- New strict internal wrappers on allocator hot paths require release assembly
  evidence before signoff. Run `scripts/capture-release-assembly.sh` and attach
  the generated `target/strict-type-assembly/` files or preserved artifact
  links. The review must cover free-list pop, local free, remote free, small
  page lookup, and any changed hot path. Reject the wrapper unless the
  before/after evidence shows no added hot-path branch, call, load, store,
  atomic memory operation, lock prefix, stack spill, formatting, allocation,
  panic path, bounds-check slow path, or wrapper-constructor traffic; otherwise
  record a measured justification and redesign if the wrapper cost is not
  intentional.
- Final strict-type completion is validated by the TODO gate: formatting,
  default library tests, the release fast-path audit, `std` feature tests,
  layout, raw facade, FFI, API inventory, safe-boundary, strict-type, lint,
  feature, and ABI symbol checks.
