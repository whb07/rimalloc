# Internal Type Capability Inventory

This inventory classifies every production wrapper declared under `src/types`.
Test-only fixture records are excluded. The classes are:

- **identity** — cheap metadata or address identity; may remain `Copy`;
- **value** — an immutable scalar, option, decision, snapshot, or checked
  request; may remain `Copy` when duplication has no ownership meaning;
- **raw ABI** — a C-layout record or enum whose traits are fixed by ABI use;
- **mechanism** — an atomic cell, encoded-link helper, callback wrapper, or
  borrowed view; its trait policy follows the underlying mechanism;
- **linear capability** — proof of state, validation, ownership, or detached
  resources; must implement neither `Copy` nor `Clone`.

`ThreadHeapLocal` is intentionally an identity despite its name: it proves
thread affinity, but does not claim exclusive ownership of every operation on
the thread heap. `ThreadFreeWord` is a value snapshot of an atomic word, not
ownership of the chain whose address it observes.

## Pointer and scalar domains

| Module | Classification | Types |
|---|---|---|
| `ptr` | identity | `PagePtr`, `PageConstPtr`, `BlockPtr`, `HeapPtr`, `ThreadHeapPtr`, `TldPtr`, `ArenaPtr`, `SubprocPtr`, `PageQueuePtr`, `UserPtr`, `InteriorUserPtr`, `PageStart`, `SliceStart`, `OsMemPtr` |
| `scalar` | value | `AllocSizeError`, `Errno`, `RequestedSize`, `EffectiveSize`, `PaddedSize`, `UsableSize`, `UsableOut`, `BlockSize`, `FullBlockSize`, `Alignment`, `MaybeAlignment`, `AlignmentOffset`, `WordCount`, `PageQueueCount`, `PageReclaimOnFree`, `PageReclaimLimit`, `UsedBlockCount`, `ReservedBlockCount`, `PageCapacity`, `RetireExpire`, `NumaNode`, `ThreadSeq`, `HeapSeq`, `SubprocSeq`, `ArenaCount`, `ThreadCount`, `HeapCount`, `Heartbeat`, `OsPageSize`, `PageMapIndex`, `PageMapSubIndex`, `Bin`, `StatsBin`, `DirectBin`, `SliceIndex`, `SliceCount`, `ArenaIndex`, `GoodSize` |
| `page` | value/identity | `PageFlags`, `ThreadId`, `PageOwnerId`, `PageThreadId`, `XThreadDelta`, `FreePathKind`, `RegisteredPageRange` |
| `freelist` | value snapshot | `ThreadFreeOwner`, `ThreadFreeWord` |
| `track` | value | `TrackMode` |

## Allocation, validation, and free-list types

| Module | Classification | Types |
|---|---|---|
| `layout` | value | `Zeroed`, `MaybeHugeAlignment`, `LayoutError`, `LayoutRequest`, `ItemCount`, `CallocRequest`, `AllocProduct`, `OldAllocation`, `PreserveBytes`, `ReallocSource`, `ReallocRequest`, `MallocGenericFlags` |
| `validation` | value | `BlockOffset`, `PointerInvalidReason`, `WasGuarded` |
| `validation` | linear capability | `GuardedBlock`, `PointerLocation`, `LocatedBlock`, `ValidPointerLookup`, `BlockForFree`, `ValidatedFree`, `ValidatedFreeParts` |
| `validation_extra` | linear capability | `AllocatedBlock`, `FreeBlock` |
| `validation` | sum result | `PointerLookup` (not `Copy`; its valid branch owns `ValidPointerLookup`) |
| `freelist_wrappers` | identity/value | `PageKeys`, `EncodedWord`, `EncodedPtr`, `BlockTag`, `AlignedBlockTag`, `GuardedBlockTag`, `BlockLink` |
| `freelist_wrappers` | linear capability | `AvailableFreeList`, `LocalFreeList`, `EncodedFreeList` |
| `atomic::thread_free` | linear capability | `PublishedRemoteHead`, `DetachedRemoteList` |
| `atomic::thread_free` | mechanism/result | `AtomicThreadFree`, `RemotePushResult` |
| `page_batch` | linear capability | `RemoteFreeBatch`, `LocalFreeBatch`, `LeasedBatch` |
| `free_actions` | value | `LocalFreeMode` |

`ValidatedFreeParts` is a one-time destructuring result and therefore follows
the linear request it contains. `PointerLookup` itself is not listed as a
capability because its null and invalid branches carry no state token, but it
must remain non-`Copy` so the valid branch cannot be duplicated.

## Page and queue state

| Module | Classification | Types |
|---|---|---|
| `page_states` | marker value | `Uninit`, `Active`, `Full`, `Retired`, `Abandoned`, `AbandonedMapped`, `Destroyed` |
| `page_states` | linear capability | `Page<S>`, `ActivePage`, `FullPage`, `RetiredPage`, `AvailablePage`, `LocalPage`, `ForeignPage`, `AbandonedPageType`, `AbandonedMappedPageType`, `OwnedAbandonedPage`, `DestroyedPage`, `AllFreePage<S>`, `PageForAbandon`, `PageAbandonOutcome`, `PageFreeOwner` |
| `page_states` | identity | `ThreadHeapLocal` |
| `page_queues` | marker value | `NormalQueue`, `HugeQueue`, `FullQueue` |
| `page_queues` | identity | `PageQueue<K>`, `NormalPageQueue`, `HugePageQueue`, `FullPageQueue` |
| `page_queues` | linear capability | `QueuedPage<K>`, `UnqueuedPage<S>` |

Marker types have no resource identity on their own. `Page<S>` and queue
membership wrappers do carry a page state proof and are linear even though
their raw representation remains pointer-sized.

## Heap, arena, memory, and atomic mechanisms

| Module | Classification | Types |
|---|---|---|
| `heap` | ownership/borrow mechanism | `MainHeap`, `OwnedHeap`, `BorrowedHeap`, `ThreadHeapBorrowed`, `ThreadHeapState`, `MainSubproc`, `BorrowedSubproc` |
| `arena` | value | `ArenaAllocation`, `SliceError`, `SliceRange`, `SharedArena`, `ExclusiveArena`, `MatchNuma`, `AllowPinned`, `Exclusive`, `ParentArena` |
| `arena` | mechanism | `ArenaPageTablePtr`, `AbandonedCountCell`, `ArenaPageCell` |
| `memid` | value/identity | `MemKindCode`, `MemKind`, `AnyMem`, `NoneMem`, `ExternalMem`, `StaticMem`, `MetaMem`, `OsMem`, `ArenaMem`, `HeapMainMem`, `MemId<M>`, `MemIdView`, `BlockIndex`, `ReservedBytes`, `CommittedBytes`, `InitiallyCommitted`, `InitiallyZero`, `Pinned`, `OsMemId`, `OsMemKind`, `ExternalMemId`, `ArenaMemId`, `MetaMemId` |
| `padding` | value/mechanism | `PaddingCanary`, `PaddingDelta`, `PaddingPtr`, `Padding` |
| `atomic` | mechanism | `AtomicAbandonedCount`, `AtomicArenaPage`, `AtomicUsizeField`, `AtomicI64Field`, `AtomicPtrField<T>`, `AtomicGuard`, `OnceStatePtr`, `OnceGuard`, `MiLockPtr`, `MiLockGuard`, `AtomicPageThreadId`, `AtomicThreadHeapRefcount` |
| `callbacks` | mechanism | `CallbackArg`, `DeferredFreeCallbackArg`, `OutputCallbackArg`, `ErrorCallbackArg`, `HeapVisitCallbackArg`, `BlockVisitCallbackArg`, `ThreadLocalClaimCallbackArg`, `DeferredFreeCallback`, `OutputCallback`, `ErrorCallback`, `HeapVisitCallback`, `BlockVisitCallback`, `ThreadLocalClaimCallback` |

Heap and arena ownership wrappers predate Stage 1 and do not appear in its
normative non-`Copy` list. They remain governed by their existing constructors
and lifecycle tests; changing them is outside Stages 0–3 unless required by a
named transition.

## Raw C-layout types

The following `constants` wrappers preserve translated C representation and
are classified as raw ABI: `mi_process_info_t`, `mi_random_ctx_t`,
`mi_page_queue_t`, `mi_page_kind_e`, `mi_block_s`, `mi_padding_s`,
`mi_heap_area_t`, `mi_arena_pages_s`, `mi_subproc_s`, `mi_tld_s`, `mi_arena_s`,
`mi_heap_s`, `mi_page_s`, `mi_theap_s`, `mi_tls_slot_t`,
`mi_thread_locals_t`, `mi_option_init_e`, `mi_option_desc_t`, and
`mi_subproc_id_t`.

Stage 1 changes no raw field, layout, symbol, or calling convention. Trait
changes apply only to transparent Rust capabilities layered over these raw
records.
