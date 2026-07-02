# Unsafe Audit

Unsafe code in this allocator is expected and must remain documented. Each
unsafe subsystem needs a short proof covering pointer validity, ownership,
alignment, initialization, aliasing, concurrency, and unwind behavior.

## Current Audit Units

- `alloc/free/realloc`: caller pointer ownership, allocation origin, usable-size
  lookups, page metadata lookup, padding checks, and realloc copy bounds.
- `Allocation: Send + Sync`: the safe wrapper uniquely owns a mimalloc
  allocation, mimalloc supports freeing from a different thread, and shared
  references expose only metadata plus raw pointers. Safe code cannot read or
  mutate the allocation contents through `&Allocation`; raw pointer dereference
  remains an unsafe caller boundary.
- `page/arena/os`: page ownership, commit/decommit/reset/purge state, arena
  slice boundaries, and OS out-parameters.
- `threadlocal/theap`: thread-local heap pointer lifetime, abandonment, reclaim,
  and cross-thread publication.
- `atomic`: acquire/release/relaxed ordering around free lists, ownership, locks,
  and once initialization.
- `types`: crate-private witnesses for pointer domains, layout requests,
  free-list ownership, page states, memid views, and atomic metadata fields.
- `track`: Valgrind/ASan hooks and feature-exclusive instrumentation.
- `raw ABI`: C callbacks, null/error conventions, exported symbols, and panic
  boundaries.

## Fuzz Finding Mapping

`docs/testing.md` defines the fuzz corpus and minimized reproducer policy. A
confirmed fuzz finding that changes an unsafe proof obligation must update this
audit document, add or update the relevant `SAFETY[AUDIT:*]` tag when a boundary
changes, and record the corpus file, regression test, sanitizer/Miri script, or
release artifact script in `docs/unsafe-audit-evidence.tsv`.

Release-audit evidence rows should cite the focused module that owns the
assertion under `tests/release_fast_path_audit/` instead of an aggregate harness
file when such a module exists.

## Boundary Audit Tags

- `SAFETY[AUDIT:global-alloc]`: the `MiMalloc` `GlobalAlloc` facade. Callers
  provide valid non-zero `Layout` values and live allocation pointers according
  to Rust's allocator contract. The callee dispatches to normal or aligned
  mimalloc paths, returns null on failure, preserves reallocation contents on
  success, and does not unwind intentionally.
- `SAFETY[AUDIT:owned-allocation]`: the safe `Allocation` wrapper. The wrapper
  owns one allocation, frees it exactly once on drop, treats zero-sized
  allocations as aligned sentinels, exposes raw pointers without transferring
  free rights, and never exposes safe references to uninitialized bytes.
- `SAFETY[AUDIT:ffi-export]`: exported C ABI functions and aliases. Callers
  must uphold the C API pointer, layout, ownership, callback, and threading
  rules. Exported functions must not unwind across the C boundary and must stay
  classified by the FFI/export boundary checks.
- `SAFETY[AUDIT:raw-callback]`: raw callback entrypoints and callback storage.
  Callers must provide ABI-compatible function pointers and user data that stay
  valid for the documented call duration.
- `SAFETY[AUDIT:raw-macro-boundary]`: public raw macro expansion. Typed raw
  allocation macros and C header override macros expand to unsafe raw C-shaped
  calls without internal unsafe blocks, so the downstream expansion site must
  provide the unsafe boundary.
- `SAFETY[AUDIT:raw-macro-allocation-boundary]`: raw allocation override facade
  in `src/raw/macros.rs`. Export attributes, C++ operator symbols, libc/WASI
  aliases, macOS interpose records, assembly overrides, and bootstrap
  allocation tracking preserve the raw C allocation ABI and do not add Rust
  ownership guarantees.
- `SAFETY[AUDIT:init-subproc-boundary]`: subproc identity, subproc list
  mutation, subproc ID conversion, main-subproc address resolution, main-heap
  lookup, and subproc heap visitation in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-process-boundary]`: process initialization and teardown in
  `src/allocator/init.rs`, including one-time state, preloading gates,
  allocator init/done callbacks, post-init option activation, and teardown
  ordering.
- `SAFETY[AUDIT:init-thread-tls-boundary]`: thread initialization/teardown,
  TLD allocation/free, default/cached thread-heap association, process-main
  thread-heap TLS, and platform TLS slot/key setup in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-options-guarded-boundary]`: thread-heap option and
  guarded-allocation configuration in `src/allocator/init.rs`, including
  clamped option reads and bounded local thread-heap writes.
- `SAFETY[AUDIT:init-random-cpu-boundary]`: CPU feature detection, CPU
  threshold publication, and random seeding/reseeding in
  `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-platform-hooks-boundary]`: hidden init link-name assembly
  in `src/allocator/init.rs` plus process attach/detach constructor and
  destructor hook glue in `src/platform/mod.rs`.
- `SAFETY[AUDIT:allocation-entry-boundary]`: allocation entry points in
  `src/allocator/alloc.rs`. The boundary covers malloc/calloc/realloc request
  construction, heap/thread-heap dispatch, zeroing, padding/provenance recovery,
  product overflow checks, raw allocation exports, and no-allocation hot paths.
- `SAFETY[AUDIT:page-allocation-boundary]`: page-local allocation in
  `src/allocator/page.rs`. The boundary covers free-list collection and
  extension, generic refill, page queue movement, retire/free/abandon
  transitions, stat gates, huge-page allocation, raw page helper exports, and
  no-allocation hot paths.
- `SAFETY[AUDIT:arena-slice-boundary]`: arena slice/range projection in
  `src/allocator/arena.rs`. The boundary covers slice indexes, range
  projection, area/containment queries, info-slice sizing, and arena bitmap
  layout.
- `SAFETY[AUDIT:arena-registry-boundary]`: arena registry and subproc table
  access in `src/allocator/arena.rs`. The boundary covers arena indexes,
  subproc arena cells, heap arena-page tables, NUMA suitability, arena
  iteration, and unsafe arena teardown.
- `SAFETY[AUDIT:arena-allocation-boundary]`: arena allocation and OS
  reservation in `src/allocator/arena.rs`. The boundary covers slice bitmap
  allocation, arena initialization, reserve growth, OS-memory fallback, public
  reserve/manage APIs, and huge-page reservation.
- `SAFETY[AUDIT:arena-abandoned-boundary]`: abandoned-page claim/reclaim in
  `src/allocator/arena.rs`. The boundary covers abandoned ownership transfer,
  heap arena-page bitmap lookup, remote-free collection, and thread-local claim
  callbacks.
- `SAFETY[AUDIT:arena-page-lifecycle-boundary]`: arena-backed page lifecycle in
  `src/allocator/arena.rs`. The boundary covers fresh/regular/singleton page
  allocation, page free/abandon/reabandon/unabandon shims, page memid
  ownership, and heap arena-page publication.
- `SAFETY[AUDIT:arena-purge-boundary]`: arena purge and collection in
  `src/allocator/arena.rs`. The boundary covers purge scheduling, purge bitmap
  traversal, arena collection/free, OS purge/decommit handoff, and purge stats.
- `SAFETY[AUDIT:internal-bitmap-boundary]`: internal bitmap and binned-bitmap
  operations in `src/internal/bitmap.rs`. The boundary covers chunk/field
  projection, bit set/clear/search loops, arena traversal callbacks, typed
  slice wrappers, and allocation-free bitmap search paths.
- `SAFETY[AUDIT:internal-translation-boundary]`: translated internal header
  helpers in `src/internal/mod.rs`. The boundary covers page-map lookup,
  page/block metadata accessors, thread-free ownership words, packed pointer
  encode/decode, tracking gates, CPU copy/fill shims, and raw C-compatible
  helper declarations.
- `SAFETY[AUDIT:windows-primitive-boundary]`: Windows primitive backend and
  translated ETW helper surface in `src/platform/windows.rs`. The boundary
  covers Windows ABI imports, dynamic primitive lookup, TLS/FLS and DLL
  lifecycle callbacks, virtual memory, process information, stderr output,
  environment copying, large/huge pages, TEB probing, and ETW provider helpers.
- `SAFETY[AUDIT:unsafe-callback]`: unsafe-adjacent callback guards. Unsafe guard construction
  registers a trampoline with a borrowed callback context, stores the previous
  registration, restores that previous registration on drop, and catches Rust
  panics before callback ABI return in `std` and test builds. Callers must
  prevent concurrent registration changes and concurrent or reentrant callback
  entry for the same borrowed `FnMut`. Output callbacks remain raw until output
  recursion and diagnostic allocation semantics are designed.
- `SAFETY[AUDIT:panic-boundary]`: no-std panic/personality shims and aborting
  assertion paths. These paths must not allocate or unwind.
- `SAFETY[AUDIT:safe-heap]`: safe heap wrappers. Owned heaps delete exactly
  once on drop, main-heap handles never delete the main heap, and heap-tied
  allocations carry the heap borrow lifetime so safe code cannot delete a heap
  while its safe allocations are live.
- `SAFETY[AUDIT:raw-auto-trait]`: retained raw C-layout auto-trait impls.
  These are limited to `Sync` for immutable sentinel static types required by
  translated C parity. Live allocator state remains raw-pointer state and must
  be accessed through allocator synchronization, raw API contracts, or narrower
  typestate wrappers.

## Safe API And FFI Boundary

- C ABI exported functions live in approved boundary files and are checked by
  `scripts/check-ffi-boundary.py`, `scripts/check-abi-symbols.sh`, and layout
  checks. They inherit C caller obligations for pointer validity, allocation
  origin, nullability, callback ABI, and thread behavior.
- The raw Rust facade is unsafe and unstable as a Rust path surface. It exposes
  C-shaped functions and layouts; callers uphold mimalloc's ownership,
  lifetime, alignment, and threading rules.
- `MiMalloc` is the only safe global allocator facade. Its unsafe trait impl is
  tagged with `SAFETY[AUDIT:global-alloc]`.
- `Allocation` and `InitializedAllocation` are owned safe allocation wrappers.
  `Allocation` has raw pointer escape hatches and no safe byte-slice access
  until initialized. `InitializedAllocation` exposes byte slices only after a
  zeroed construction path or explicit unsafe initialization assertion.
- `Heap`, `HeapRef`, `MainHeap`, `HeapAllocation`, and
  `InitializedHeapAllocation` encode heap ownership and heap-allocation
  lifetimes without exposing raw deletion rights. They are intentionally not
  `Send` or `Sync` because safe heap operations use heap/thread-heap metadata
  that is not modeled as a thread-safe Rust handle.
- `GlobalOptions`, `StatsSnapshot`, and `ProcessInfo` wrap supported allocator
  controls and observations without raw out-pointers in safe caller code.
- `unsafe_callbacks::ErrorCallbackGuard` and
  `unsafe_callbacks::DeferredFreeCallbackGuard` prevent dangling callback
  contexts with Rust borrows, restore previous callback registrations on drop,
  and catch callback panics before ABI return in `std` and test builds. Their
  constructors are unsafe because mimalloc registers process-global callbacks
  and upstream does not provide a single-threaded, non-reentrant guarantee
  strong enough for Tier S borrowed `FnMut` mutation.

## Callback Threading And Reentry Audit

- Error callbacks: upstream declares `mi_register_error` in
  `mimalloc-3.3.2/include/mimalloc.h:186-187`, stores the process-global
  `mi_error_handler` in `mimalloc-3.3.2/src/options.c:579,604-606`, and invokes
  it from `_mi_error_message` in `mimalloc-3.3.2/src/options.c:609-620`.
  The Rust translation stores the handler behind `mi_error_handler_lock` for
  registration in `src/options.rs:585-629` and invokes a loaded handler in
  `src/options.rs:1086-1111`. These locations do not restrict invocation to the
  registering thread or rule out another error callback becoming active while a
  borrowed callback is live.
- Deferred-free callbacks: upstream declares `mi_register_deferred_free` in
  `mimalloc-3.3.2/include/mimalloc.h:180-181`, explicitly notes that programs
  should install a single deferred-free handler before allocation in
  `mimalloc-3.3.2/src/page.c:888`, stores callback and argument in atomic
  globals in `mimalloc-3.3.2/src/page.c:889-905`, and invokes the callback from
  `_mi_deferred_free` in `mimalloc-3.3.2/src/page.c:892-901`. The Rust
  translation mirrors the atomic global registration and invocation in
  `src/allocator/page/deferred.rs`. Invocation is suppressed for reentry on a given
  thread heap via `theap->tld->recurse`, but the process-global callback can be
  reached by allocator activity on different thread heaps.
- Decision: borrowed `FnMut` callback guards cannot remain safe under the
  documented upstream contract. Construction is unsafe and requires callers to
  serialize registration, restoration, and callback entry for the borrowed
  closure. Nested guards must be dropped in LIFO order because each guard
  restores the process-global snapshot it captured during construction. The
  trampolines retain panic containment for `std` and test builds; optimized
  `no_std` callers must not panic from callbacks.
- `error_callback_trampoline::<F>` is registered only by
  `ErrorCallbackGuard::new`. `arg` must be the non-null `&mut F` pointer stored
  by the guard for the active registration lifetime; the trampoline returns
  early for null, catches panics in `std` and test builds, and otherwise invokes
  the borrowed callback under the caller's no-concurrent-entry obligation.
- `deferred_free_callback_trampoline::<F>` is registered only by
  `DeferredFreeCallbackGuard::new`. `arg` must be the non-null `&mut F` pointer
  stored by the guard for the active registration lifetime; `force` and
  `heartbeat` are forwarded exactly from mimalloc, panics are caught in `std`
  and test builds, and no concurrent or reentrant entry may access the same
  borrowed callback.
- Raw pointer escape hatches from safe types must document invalidation, null or
  sentinel behavior, caller free restrictions, use-after-move/drop behavior,
  aliasing, and thread restrictions.

## Raw Allocator Auto-Trait Audit

- Removed raw `Send` impls from `mi_tld_s`, `mi_page_s`, `mi_theap_s`,
  `mi_arena_pages_s`, `mi_subproc_s`, `mi_arena_s`, `mi_heap_s`,
  `mi_tls_slot_t`, `mi_thread_locals_t`, and `mi_option_desc_t`. Safe
  allocation wrappers keep their own narrower `Send`/`Sync` proofs.
- Removed raw `Sync` impls from `mi_tld_s`, `mi_arena_pages_s`,
  `mi_subproc_s`, `mi_arena_s`, `mi_heap_s`, `mi_tls_slot_t`,
  `mi_thread_locals_t`, and `mi_option_desc_t`; no current safe wrapper or
  translated static requires those raw layout types to be shareable through
  safe references.
- Retained `Sync` for `mi_page_s` because `src/allocator/init.rs` exposes the immutable
  `_mi_page_empty` sentinel static as a translated C compatibility object. Live
  pages are still allocator-owned mutable metadata and are not made safe to
  share by this impl.
- Retained `Sync` for `mi_theap_s` because `src/allocator/init.rs` exposes immutable
  empty thread-heap sentinel statics. Live thread heaps remain thread-owned
  allocator state and are accessed through raw allocator paths or typestate
  wrappers.
- Unit tests in `src/types/` assert the intentionally retained raw `Sync`
  impls so accidental removals are visible during routine builds. Source search
  for `unsafe impl Send` in `src/types/` should remain empty.

## Policy

- Prefer local unsafe blocks with nearby safety reasoning over broad
  `unsafe_op_in_unsafe_fn` allowances.
- Do not add module-level `allow(unsafe_op_in_unsafe_fn)`.
- Any new unsafe wrapper must document caller obligations.
- Any new safe wrapper around raw allocator state must explain how it enforces
  ownership, lifetime, and aliasing invariants.
- `scripts/check-safe-boundary.py` enforces that the safe facade has only
  allowlisted public unsafe Rust items and that boundary unsafe blocks in the
  safe facade carry stable audit tags.
