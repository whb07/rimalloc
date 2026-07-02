# Unsafe Audit

Scope: Rust sources in `src/` that use `unsafe`, raw pointers, exported C ABI
entry points, or allocator metadata structs. This audit is for the supported
default parity matrix documented in `PARITY.md`; debug, secure, guarded, and
platform-specific override gaps remain tracked by their own TODO items.

The Rust port is an allocator replacement and necessarily keeps the C layout and
pointer-oriented control flow. The unsafe surface is acceptable under these
shared invariants:

- Public `extern "C"` APIs accept the same nullable and invalid-pointer inputs as
  mimalloc C, validate where the C implementation validates, and otherwise
  forward to internal helpers with the C ABI contract.
- Metadata pointers are obtained from owning allocator structures, page maps, or
  OS mappings before dereference. Interior metadata access follows the same page,
  segment, arena, and heap ownership rules as mimalloc 3.3.2.
- Raw pointer arithmetic is limited to C-equivalent block/page/arena layout
  calculations and is guarded by size-class, alignment, overflow, and bounds
  checks in the allocation, page-map, and OS helpers.
- Cross-thread metadata mutation goes through the audited atomic helpers, page
  ownership checks, delayed-free queues, abandonment/reclaim paths, or explicit
  locks.
- Callback invocation and process/thread lifecycle entry points keep the same
  initialization and reentrancy constraints as C mimalloc, with documented
  intentional divergences in `PARITY.md`.

Automated evidence is tracked in `docs/unsafe-audit-evidence.tsv`. When a
release-audit assertion lives in a focused module under
`tests/release_fast_path_audit/`, the evidence row should name that focused
module instead of the root harness or aggregate module shim.

Per-file review:

- `src/abi.rs`: raw declarations expose the C ABI surface without adding
  allocator ownership assumptions.
- `src/alloc.rs`: allocation fast paths dereference heaps, theaps, pages, and
  free-list blocks after initialization and size-class selection. Usable-size
  out-pointers are checked for null before writes. Overflow-prone count/size
  calls use existing checked helpers.
- `src/alloc_aligned.rs`: aligned allocation/reallocation uses C-equivalent
  offset and alignment checks before computing adjusted pointers; the original
  allocation pointer is preserved through mimalloc metadata before free.
- `src/alloc_override.rs`: override shims expose C/C++ ABI entry points and
  forward to public allocation APIs. Unsafe operations are FFI boundary calls and
  symbol interposition wrappers; ownership remains with the allocator APIs.
- `src/alloc_posix.rs`: POSIX and libc-compatible APIs validate null output
  pointers, alignment constraints, overflow, and errno-style failures before
  writing through caller pointers.
- `src/arena.rs`: arena metadata pointers are derived from registered arena
  ranges and bitmap/page ownership checks. Slice allocation, purge, commit, and
  free paths update shared state through audited atomics and locks.
- `src/allocator/arena/debug.rs`: arena diagnostics read allocator-owned arena,
  page, bitmap, and memid metadata selected from the current sub-process arena
  registry and write only stack-local render buffers before sending text to the
  raw diagnostic output path.
- `src/allocator/arena/ids.rs`: arena id and memid helpers own the raw
  arena-id shims, top-level arena validation, typed memid range extraction, and
  page-size classification used by arena allocation/free paths.
- `src/allocator/arena/heap_pages.rs`: heap page visitors, public heap visit
  callbacks, and heap page move/destroy helpers preserve C ABI traversal while
  keeping page ownership transfer inside the arena module boundary.
- `src/allocator/arena/prefix.rs`: arena, heap, and sub-process prefix layouts
  remain private to `allocator::arena`; accessors require typed heap/subproc
  witnesses or checked arena/bin indexes before returning typed atomic fields.
- `src/allocator/arena_meta.rs`: arena metadata page stack operations publish
  and reclaim metadata pages through the audited atomic stack discipline.
- `src/atomic.rs`: raw atomic pointer casts are centralized here and mapped to
  C mimalloc atomic operations. Ordering equivalence is covered by
  `ATOMIC_ORDERING_AUDIT.md`.
- `src/allocator/bench_diagnostics.rs`: unsafe diagnostic hooks are
  test/benchmark support and do not mutate allocator metadata outside the
  measured path.
- `src/bitmap.rs`: bitmap word pointers are derived from caller-provided bitmap
  storage and bounded by bit counts; set/clear/claim transitions use atomic word
  operations equivalent to C.
- `src/core_types/arena.rs`, `src/core_types/atomic_wrappers.rs`,
  `src/core_types/callbacks.rs`, `src/core_types/freelist.rs`,
  `src/core_types/freelist_wrappers.rs`, `src/core_types/guarded.rs`,
  `src/core_types/heap.rs`, `src/core_types/layout.rs`,
  `src/core_types/memid.rs`, `src/core_types/padding.rs`,
  `src/core_types/page.rs`, `src/core_types/page_queues.rs`,
  `src/core_types/page_states.rs`, `src/core_types/ptr.rs`,
  `src/core_types/scalar.rs`, `src/core_types/validation.rs`, and
  `src/core_types/validation_extra.rs`: typed newtypes and validation helpers
  confine raw metadata, page, heap, callback, freelist, and scalar invariants to
  small wrappers used by the translated allocator paths.
- `src/types/padding.rs`: padding metadata pointer arithmetic and field access
  are confined to `PaddingPtr`, `PaddingCanary`, and `PaddingDelta`; callers
  supply allocator-owned block padding boundaries and receive typed values.
- `src/free.rs`: free paths recover page metadata from owned block pointers,
  validate page ownership where C does, and route remote frees through delayed
  free queues instead of mutating foreign-thread page state directly.
- `src/headers.rs`: exported compatibility macros preserve the C header surface
  and forward unsafe pointer use to the raw allocator APIs.
- `src/heap.rs`: heap creation/deletion/collection touches heap and theap
  metadata only after initialization checks and mirrors the C ownership transfer
  rules for live blocks and abandoned pages.
- `src/init.rs`: global and thread-local initialization uses explicit once/lock
  state before publishing allocator globals. Destructor paths tolerate partial
  initialization as in C.
- `src/internal.rs`: low-level helpers implement C layout arithmetic for pages,
  blocks, encoded free-list links, memids, and memcpy/memset wrappers. Callers
  are responsible for passing allocator-owned pointers; helpers are small and
  C-invariant preserving.
- `src/internal/memid.rs`: raw memory-origin identifiers keep the C-compatible
  `mi_memid_t` layout and route construction/classification through
  `crate::core_types::MemId` typed views.
- `src/lib.rs`: crate-level exported allocator configuration keeps unsafe global
  allocator wiring at the public boundary.
- `src/libc.rs`: libc FFI calls are thin wrappers around platform C APIs with
  C-compatible argument passing.
- `src/mimalloc.rs`: declarations expose the public ABI surface for tests and
  consumers; unsafe is limited to foreign function declarations.
- `src/options.rs`: option, output, and error callback pointers are stored and
  invoked using C-compatible types; null checks and atomic publication guard
  concurrent reads.
- `src/os.rs`: OS memory management keeps base/size/alignment information in
  memids and validates commit/decommit/reset/protect ranges before platform
  primitive calls.
- `src/page.rs`: page initialization, free-list construction, retirement,
  abandonment, and reclaim paths operate on pages selected by heap/theap queues
  and size classes. Encoded block links and thread-free state follow C page
  invariants.
- `src/allocator/page/flags.rs`: page flag, full-queue, huge-page,
  abandoned-page, and thread-heap owner helpers route raw page metadata through
  typed page, flag, memid, and atomic owner wrappers.
- `src/allocator/page/validation.rs`: page validation and block lookup helpers
  route raw page/block metadata through typed page witnesses before checking
  capacity, free-list, queue-membership, and ownership invariants.
- `src/allocator/page/bins.rs`: size-class and good-size helpers preserve the
  C bin ABI wrappers while routing internal callers through typed `Bin` and
  `GoodSize` domains.
- `src/allocator/page/counts.rs`: page and page-queue count accessors read and
  update allocator-owned metadata through typed counter wrappers while
  preserving the original inline helper shape.
- `src/allocator/page/deferred.rs`: deferred-free callback registration and
  dispatch owns the process-global callback slots and increments the local
  thread heap heartbeat before invoking the callback under the C-compatible
  reentry guard.
- `src/page_map.rs`: page-map submap publication and lookup bounds-check indexes
  against the C page-map layout and uses audited atomics for concurrent install.
- `src/prim.rs`: primitive facade forwards to platform implementations through
  C-compatible signatures.
- `src/prim/emscripten.rs`: Emscripten primitive calls preserve C ABI contracts
  and validate OS-memory result pointers before publishing memids.
- `src/prim/osx.rs`: malloc-zone structs and callbacks match macOS ABI layout;
  zone operations forward to allocator APIs or documented no-op compatibility
  hooks.
- `src/prim/unix.rs`: Unix primitive calls wrap `mmap`, `munmap`, `madvise`,
  `mprotect`, pthread TLS, environment, and entropy APIs with C-compatible
  pointer and size handling.
- `src/prim/wasi.rs`: WASI primitive calls expose the same allocator primitive
  shape; unsupported operations return C-compatible failure values instead of
  dereferencing invalid state.
- `src/prim/windows.rs`: Windows primitive calls wrap `VirtualAlloc`,
  `VirtualFree`, TLS/FLS, environment, and large-page APIs while keeping
  allocator-owned range metadata in memids.
- `src/random.rs`: entropy buffer writes use slice creation only after checking
  for null and length, and random contexts are caller-owned allocator metadata.
- `src/provenance.rs`: pointer-address helpers preserve provenance-sensitive
  conversions in one small module used by allocator layout code.
- `src/raw.rs`: raw module re-exports preserve the C ABI contract for callers.
- `src/safe.rs`: safe facade wrappers forward to raw allocator APIs while keeping
  unsafe pointer use at the boundary.
- `src/safe/allocations.rs`, `src/safe/heaps.rs`, and `src/safe/stats.rs`:
  Rust-facing wrappers keep unsafe allocator, heap, and statistics calls behind
  documented ownership and lifetime boundaries.
- `src/safe/tests.rs`: safe-facade tests exercise the unsafe wrapper boundaries
  under serialized process-global callback and option state without widening the
  production safe API.
- `src/unsafe_callbacks.rs`: unsafe-adjacent borrowed callback guards keep
  process-global callback registration, restoration, panic containment, and
  borrowed `FnMut` lifetime requirements in an explicit namespace.
- `src/stats.rs`: stats pointers are null-checked for public getters, and
  internal merges operate on allocator-owned stats structures. Callback output
  uses C-compatible function pointers.
- `src/theap.rs`: theap/page visitors, collection, creation, deletion, and heap
  association dereference metadata after initialization checks and lock where C
  locks linked theap lists.
- `src/threadlocal.rs`: thread-local slot storage is allocated/reallocated with
  explicit layouts, versioned for freshness, and freed only through the owning
  TLS bookkeeping path.
- `src/track.rs`: tracking hooks are no-op compatibility shims in the default
  matrix and forward to the C tracking shim when `track-valgrind` or
  `track-asan` is enabled. Full Valgrind/ASAN report parity is guarded by
  `scripts/check-valgrind-tracking-parity.py` and
  `scripts/check-asan-tracking-parity.py`.
- `src/types.rs`: unsafe `Send`/`Sync` impls preserve the allocator's C
  concurrency model: mutation is protected by ownership, atomics, or locks; raw
  callback types are ABI declarations only.
- Nested core-type tests with unsafe fixtures exercise wrapper invariants
  without widening the production API:
  `src/core_types/arena/tests.rs`,
  `src/core_types/atomic_wrappers/tests.rs`,
  `src/core_types/callbacks/tests.rs`,
  `src/core_types/freelist/tests.rs`,
  `src/core_types/freelist_wrappers/tests.rs`,
  `src/core_types/heap/tests.rs`,
  `src/core_types/layout/tests.rs`,
  `src/core_types/memid/tests.rs`,
  `src/core_types/padding/tests.rs`,
  `src/core_types/page/tests.rs`,
  `src/core_types/page_queues/tests.rs`,
  `src/core_types/page_states/tests.rs`,
  `src/core_types/ptr/tests.rs`,
  `src/core_types/scalar/tests.rs`, and
  `src/core_types/validation/tests.rs`.

Operation IDs:

- `SAFETY[AUDIT:c-header-macro-reentry]`: `src/ffi/abi.rs` exposes C header
  compatibility macros whose expansion may reenter allocator APIs. The macros
  preserve C surface behavior and keep unsafety at the raw ABI call boundary.
- `SAFETY[AUDIT:ffi-abi-declarations]`: `src/ffi/abi.rs` owns the
  declaration-only raw mimalloc C ABI block. The block does not call into C or
  dereference pointers; it preserves upstream nullable raw pointer signatures so
  each unsafe call site inherits the documented C ABI ownership and lifetime
  obligations.
- `SAFETY[AUDIT:ffi-libc-string-boundary]`: `src/ffi/libc.rs` owns libc-like
  C-string helpers for bounded scans, comparisons, copies, concatenation, and
  substring lookup. Callers provide readable nul-terminated strings and writable
  destination ranges where required.
- `SAFETY[AUDIT:ffi-libc-env-boundary]`: `src/ffi/libc.rs` owns environment
  lookup forwarding. Unsafe operations are limited to validating nullable
  C-string and output-buffer arguments before calling the platform primitive.
- `SAFETY[AUDIT:ffi-libc-output-boundary]`: `src/ffi/libc.rs` owns the local
  numeric output helper used by fixed-buffer formatting. Unsafe operations are
  limited to one-byte writes and pointer advancement inside the caller-provided
  output range.
- `SAFETY[AUDIT:ffi-libc-format-boundary]`: `src/ffi/libc.rs` owns
  fixed-buffer formatting, Unix `vsnprintf` forwarding, and the non-Unix
  fallback formatter. Callers provide writable output buffers and valid varargs
  objects where platform forwarding is used.
- `SAFETY[AUDIT:ffi-libc-retention-boundary]`: `src/ffi/libc.rs` owns no-call
  retention pointers that preserve C ABI helper symbols and function pointer
  shapes without invoking the helpers.
- `SAFETY[AUDIT:global-alloc]`: `src/lib.rs` implements `GlobalAlloc` for the
  zero-sized `MiMalloc` facade. Rust's allocator contract supplies the layout,
  pointer, and ownership preconditions, and each operation forwards directly to
  the matching allocator entry point.
- `SAFETY[AUDIT:heap-thread-association-boundary]`: `src/allocator/heap.rs`
  owns heap-to-thread-heap lookup and lazy per-heap thread-heap initialization.
  Unsafe operations are limited to nullable heap metadata, thread-local heap
  slots, default/main thread-heap fallbacks, and cached thread-heap publication.
- `SAFETY[AUDIT:heap-configuration-boundary]`: `src/allocator/heap.rs` owns
  heap NUMA affinity mutation. Unsafe operations are limited to nullable heap
  normalization, OS NUMA count reads, and typed heap field updates.
- `SAFETY[AUDIT:heap-stats-merge-boundary]`: `src/allocator/heap.rs` owns
  heap-to-main stats merge shims. Unsafe operations are limited to borrowed heap
  witnesses and raw stats pointers derived from live heap metadata.
- `SAFETY[AUDIT:heap-creation-boundary]`: `src/allocator/heap.rs` owns public
  heap creation. Unsafe operations initialize the allocator thread state,
  allocate zeroed heap metadata from the main heap, create a thread-local slot,
  and link the heap into the subproc list under its lock.
- `SAFETY[AUDIT:heap-nonmain-cleanup-boundary]`: `src/allocator/heap.rs` owns
  final non-main heap cleanup. Unsafe operations walk and free heap-local
  thread heaps, release arena page tables and thread-local slots, unlink the
  heap from its subproc, tear down locks, and free heap metadata.
- `SAFETY[AUDIT:heap-nonmain-lifecycle-boundary]`: `src/allocator/heap.rs`
  owns non-main heap delete and destroy entry points. Unsafe operations reject
  main heaps, move or destroy owned pages as required, and delegate final
  cleanup through typed `OwnedHeap` witnesses.
- `SAFETY[AUDIT:heap-main-lifecycle-boundary]`: `src/allocator/heap.rs` owns
  main-heap force-destroy handling. Unsafe operations classify live heap
  metadata, preserve main-heap ownership, and destroy pages/thread heaps without
  freeing the static main heap record.
- `SAFETY[AUDIT:heap-ownership-query-boundary]`: `src/allocator/heap.rs` owns
  allocator page ownership queries. Unsafe operations are limited to safe page
  lookup, nullable heap selection, borrowed/owned heap witness construction, and
  raw C ABI ownership shims.
- `SAFETY[AUDIT:heap-underutilized-boundary]`: `src/allocator/heap.rs` owns
  under-utilized page inspection. Unsafe operations convert raw heap/page
  arguments into typed witnesses, read page counters, and compare optional heap
  ownership without retaining pointers.
- `SAFETY[AUDIT:thread-heap-visit-boundary]`:
  `src/allocator/thread_heap.rs` owns thread-heap area, page, and block
  visitation. Unsafe operations are limited to C ABI callbacks, visitor argument
  slots, page/queue traversal, fast-divisor out-pointers, and typed heap/page
  witnesses during the walk.
- `SAFETY[AUDIT:thread-heap-collect-boundary]`:
  `src/allocator/thread_heap.rs` owns page and heap collection. Unsafe
  operations validate initialized thread heaps, visit pages for collect/abandon,
  run deferred free and arena collection, and merge thread-heap stats.
- `SAFETY[AUDIT:thread-heap-default-boundary]`:
  `src/allocator/thread_heap.rs` owns default thread-heap get/set shims. Unsafe
  operations are limited to initialized TLS-backed default thread heaps and
  raw C ABI compatibility.
- `SAFETY[AUDIT:thread-heap-create-boundary]`:
  `src/allocator/thread_heap.rs` owns thread-heap init and create paths. Unsafe
  operations allocate metadata, initialize random/options/guarded state, set
  refcounts and heap/TLD links, and publish list nodes under heap and TLD locks.
- `SAFETY[AUDIT:thread-heap-random-boundary]`:
  `src/allocator/thread_heap.rs` owns thread-heap random-number helper access.
  Unsafe operations read initialized thread-heap random state through typed
  local thread-heap witnesses.
- `SAFETY[AUDIT:thread-heap-refcount-boundary]`:
  `src/allocator/thread_heap.rs` owns thread-heap refcount and backing-memory
  release helpers. Unsafe operations classify memid ownership, adjust atomic
  refcounts, update stats, and free metadata through the matching allocator
  backend only when the final reference is released.
- `SAFETY[AUDIT:thread-heap-delete-boundary]`:
  `src/allocator/thread_heap.rs` owns thread-heap free/delete. Unsafe
  operations exchange heap ownership, merge stats, unlink heap/TLD list nodes
  under the requested locks, abandon pages, and clear metadata before decref.
- `SAFETY[AUDIT:init-subproc-identity-boundary]`: `src/allocator/init.rs` owns
  subproc ID conversion and current/main subproc lookup. Unsafe operations are
  limited to caller-proven subproc IDs, typed subproc witness construction, raw
  default thread-heap inspection, and opaque ID mapping without forcing process
  initialization.
- `SAFETY[AUDIT:init-main-subproc-lifecycle-boundary]`: `src/allocator/init.rs`
  owns process-main subproc address resolution and static main subproc/main heap
  publication. Unsafe operations are limited to hidden local symbol address
  resolution, static metadata initialization, and one-time main heap/subproc
  association.
- `SAFETY[AUDIT:init-subproc-registry-boundary]`: `src/allocator/init.rs` owns
  subproc creation, destruction, list mutation, and metadata cleanup. Unsafe
  operations are limited to lock-protected registry updates, metadata allocation
  and free, arena/page-map teardown handoff, and heap destruction ordering.
- `SAFETY[AUDIT:init-subproc-heap-association-boundary]`:
  `src/allocator/init.rs` owns subproc-to-main-heap lookup, heap-to-subproc
  lookup, main heap predicates, and subproc heap visitor dispatch. Unsafe
  operations are limited to typed subproc/main-heap witness construction,
  inline C-equivalent prefix loads, lazy main heap initialization, and
  lock-protected heap visitation.
- `SAFETY[AUDIT:init-subproc-stats-count-boundary]`: `src/allocator/init.rs`
  owns subproc thread/heap counters and stats merges. Unsafe operations are
  limited to typed subproc metadata access, stats header initialization,
  per-subproc thread sequence/count updates, and merge into the main subproc
  during teardown.
- `SAFETY[AUDIT:init-process-boundary]`: `src/allocator/init.rs` owns
  process-global initialization and teardown. Unsafe operations are limited to
  one-time init state, preloading gates, allocator init/done callbacks,
  process-global metadata publication, post-init option activation, stats/OS
  setup ordering, and process teardown while preserving idempotent
  initialization and no-unwind lifecycle behavior.
- `SAFETY[AUDIT:init-thread-identity-boundary]`: `src/allocator/init.rs` owns
  current-thread identity checks used by main-thread lifecycle decisions.
- `SAFETY[AUDIT:init-thread-done-entry-boundary]`: `src/allocator/init.rs`
  owns the public and internal thread-done entry points, including default
  thread-heap fallback resolution.
- `SAFETY[AUDIT:init-thread-done-dispatch-boundary]`: `src/allocator/init.rs`
  owns typed thread-done dispatch, local thread ownership checks, stat updates,
  and handoff to thread-heap cleanup.
- `SAFETY[AUDIT:init-thread-init-default-heap-boundary]`:
  `src/allocator/init.rs` owns default thread-heap initialization for main and
  non-main thread entry.
- `SAFETY[AUDIT:init-thread-done-cleanup-boundary]`: `src/allocator/init.rs`
  owns thread-local heap cleanup, abandon/free loops, cleanup waiting, and
  default/cached/main TLS reset during thread teardown.
- `SAFETY[AUDIT:init-thread-init-entry-boundary]`: `src/allocator/init.rs`
  owns public thread initialization entry, process init dependency, initialized
  checks, and thread stat increases.
- `SAFETY[AUDIT:init-thread-heap-empty-sentinel-boundary]`:
  `src/allocator/init.rs` owns the statically allocated empty thread-heap
  sentinel exposed to TLS helpers.
- `SAFETY[AUDIT:init-thread-heap-main-tls-boundary]`: `src/allocator/init.rs`
  owns process-main thread-heap TLS lookup and publication, including Linux
  initial-exec accessors and C TLS shim routing.
- `SAFETY[AUDIT:init-thread-heap-main-static-boundary]`:
  `src/allocator/init.rs` owns one-time static main thread-heap metadata
  initialization.
- `SAFETY[AUDIT:init-thread-default-heap-association-boundary]`:
  `src/allocator/init.rs` owns default thread-heap association, local ownership
  checks, TLD initialization, and cached/main TLS synchronization.
- `SAFETY[AUDIT:init-thread-cached-heap-association-boundary]`:
  `src/allocator/init.rs` owns cached thread-heap association and reset paths.
- `SAFETY[AUDIT:init-thread-heap-safe-access-boundary]`:
  `src/allocator/init.rs` owns safe exported accessors that route through
  current-thread default/cached heap TLS cells.
- `SAFETY[AUDIT:init-platform-tls-boundary]`: `src/allocator/init.rs` owns
  platform TLS slot/key setup and teardown. Unsafe operations are limited to
  one-time Windows TLS slot allocation/free, pthread key creation/deletion, and
  process-global TLS metadata mutation.
- `SAFETY[AUDIT:init-tld-lifecycle-boundary]`: `src/allocator/init.rs` owns TLD
  allocation/free and per-thread TLD metadata. Unsafe operations are limited to
  typed TLD/subproc witnesses, main/non-main TLD setup, threadpool flag
  mutation, subproc thread counters, and metadata cleanup.
- `SAFETY[AUDIT:init-options-guarded-boundary]`: `src/allocator/init.rs` owns
  thread-heap option and guarded-allocation configuration. Unsafe operations
  are limited to validated local thread-heap writes, clamped option reads,
  guarded sample/size configuration, and non-guarded compiler fences while
  preserving option/stat initialization order.
- `SAFETY[AUDIT:init-random-cpu-boundary]`: `src/allocator/init.rs` owns CPU
  feature detection, CPU threshold publication, and process random
  seeding/reseeding. Unsafe operations are limited to CPUID register writes,
  hidden CPU threshold statics, popcount publication, and weak random
  reinitialization while preserving random reseeding and CPU feature ordering.
- `SAFETY[AUDIT:init-platform-hooks-boundary]`: `src/allocator/init.rs` and
  `src/platform/mod.rs` own platform lifecycle hook glue. Unsafe operations
  are limited to hidden local link-name assembly, process attach/detach C ABI
  shims, and ELF constructor/destructor callback slots that forward into the
  bounded process lifecycle.
- `SAFETY[AUDIT:allocation-platform-shim-boundary]`:
  `src/allocator/alloc.rs` owns allocation-local platform imports and errno
  shims. Unsafe operations are limited to C ABI declarations for abort,
  pathconf/realpath, and platform errno slots while preserving libc-compatible
  failure reporting.
- `SAFETY[AUDIT:allocation-product-entry-boundary]`:
  `src/allocator/alloc.rs` owns product-sized allocation entry shims for
  calloc, mallocn, and usable-size allocation exports. Unsafe operations are
  limited to checked count/size products, typed request construction,
  caller-provided usable-size out-pointers, and raw public allocation wrappers
  while preserving overflow and null/failure semantics.
- `SAFETY[AUDIT:allocation-heap-dispatch-boundary]`:
  `src/allocator/alloc.rs` owns thread-heap and heap allocation dispatch for
  small, zeroed, calloc, and typed allocation requests. Unsafe operations are
  limited to typed heap/thread-heap witnesses, usable-size out-pointers, page
  free-list inspection, and tracking handoff while preserving no-allocation
  hot paths.
- `SAFETY[AUDIT:allocation-layout-request-boundary]`:
  `src/allocator/alloc.rs` owns allocation request construction, layout
  padding, realloc-size validation, and old-block usable-size recovery. Unsafe
  operations are limited to caller-proven allocation pointers, typed layout
  witnesses, and bounded metadata queries while preserving realloc copy bounds.
- `SAFETY[AUDIT:allocation-realloc-entry-boundary]`:
  `src/allocator/alloc.rs` owns raw, thread-heap, and heap realloc-family entry
  shims. Unsafe operations are limited to caller-proven allocation pointers,
  typed old-allocation validation results, free-on-failure behavior, zero-tail
  routing, and usable-size out-pointers while preserving C realloc-family
  semantics.
- `SAFETY[AUDIT:allocation-string-path-boundary]`:
  `src/allocator/alloc.rs` owns strdup, strndup, and realpath allocation entry
  shims. Unsafe operations are limited to caller-proven C string/path pointers,
  bounded scans, heap/default thread-heap routing, path-max caching, and
  caller-owned output buffers while preserving C string and realpath behavior.
- `SAFETY[AUDIT:allocation-cxx-new-boundary]`:
  `src/allocator/alloc.rs` owns C++ new/new_nothrow compatibility entry shims
  and retry loops. Unsafe operations are limited to optional new-handler lookup
  and invocation, exported retry shims, aligned/new realloc wrappers, and
  retained function-pointer shapes while preserving C++ abort/nothrow behavior.
- `SAFETY[AUDIT:allocation-page-refill-boundary]`:
  `src/allocator/alloc.rs` owns page refill handoff, generic fallback
  allocation, guarded allocation, zeroing, and padding canary publication.
  Unsafe operations are limited to page/block metadata reached through
  allocator ownership, active-page witnesses, refill helpers, and guarded block
  state while preserving page ownership and zeroing guarantees.
- `SAFETY[AUDIT:page-free-list-refill-boundary]`:
  `src/allocator/page.rs` owns page-local free-list collection, extension,
  search, and refill. Unsafe operations are limited to caller-proven page and
  block pointers, typed free-list witnesses, capacity counters, commit growth,
  and refill helpers while preserving block ownership and no-allocation
  hot-path constraints.
- `SAFETY[AUDIT:page-queue-full-transition-boundary]`:
  `src/allocator/page.rs` owns full/active page transitions. Unsafe operations
  are limited to typed active/full page witnesses and queue membership checks
  while preserving heap ownership.
- `SAFETY[AUDIT:page-queue-free-release-boundary]`:
  `src/allocator/page.rs` owns all-free page removal and arena release handoff.
  Unsafe operations remove active/full pages from queues, clear page metadata,
  and free through arena ownership witnesses.
- `SAFETY[AUDIT:page-queue-retire-boundary]`:
  `src/allocator/page.rs` owns page retire and retired-page collection. Unsafe
  operations update retire counters, queue front caches, and thread-heap retire
  ranges under the owning thread heap.
- `SAFETY[AUDIT:page-queue-abandon-boundary]`:
  `src/allocator/page.rs` owns active/full page abandon transitions. Unsafe
  operations detach pages from queues and publish arena abandon state while
  preserving same-theap reclaim information.
- `SAFETY[AUDIT:page-queue-reclaim-boundary]`:
  `src/allocator/page.rs` owns abandoned-page reclaim. Unsafe operations
  reattach reclaimed pages to target thread-heap queues and collect remote
  frees before normal page use resumes.
- `SAFETY[AUDIT:page-queue-validation-boundary]`:
  `src/allocator/page.rs` owns page queue containment and validation walks.
  Unsafe operations inspect linked queue nodes and block-size identity without
  mutating ownership.
- `SAFETY[AUDIT:page-queue-direct-cache-boundary]`:
  `src/allocator/page.rs` owns direct-page cache publication from normal queue
  fronts for small pages.
- `SAFETY[AUDIT:page-queue-link-mutation-boundary]`:
  `src/allocator/page.rs` owns queue link insertion, removal, and cross-queue
  enqueue. Unsafe operations update first/last pointers, page links, queue
  counts, thread-heap page counts, and in-full state together.
- `SAFETY[AUDIT:page-queue-append-boundary]`:
  `src/allocator/page.rs` owns page queue append. Unsafe operations splice
  linked page lists, retarget the owning thread heap, and merge queue counts.
- `SAFETY[AUDIT:page-huge-allocation-boundary]`:
  `src/allocator/page.rs` owns huge-page selection and allocation dispatch.
  Unsafe operations are limited to caller-proven heap and huge-queue pointers,
  typed layout/alignment witnesses, and arena page allocation handoff while
  preserving singleton page layout, alignment, and huge allocation stats.
- `SAFETY[AUDIT:page-stats-public-boundary]`:
  `src/allocator/page.rs` owns page metadata accessors, stats/bin helpers,
  public C-shaped page helpers, and generic allocation entry shims. Unsafe
  operations are limited to caller-proven page/heap pointers, typed count/bin
  witnesses, usable-size out-pointers, and ABI flag decoding while preserving
  public null/failure semantics and typed metadata invariants.
- `SAFETY[AUDIT:arena-slice-boundary]`: `src/allocator/arena.rs` owns arena
  slice/range projection, arena area queries, containment checks, info-slice
  sizing, and arena bitmap layout. Unsafe operations are limited to
  caller-proven arena pointers, typed `SliceIndex`/`SliceRange` witnesses,
  bounded metadata projection, and address containment arithmetic while
  preserving arena bounds and slice alignment.
- `SAFETY[AUDIT:arena-registry-boundary]`: `src/allocator/arena.rs` owns
  subproc arena tables, arena indexes, arena iteration, heap arena-page table
  lookup, NUMA suitability, and unsafe arena teardown. Unsafe operations are
  limited to typed arena/subproc/heap pointers, `ArenaIndex` construction,
  atomic arena table cells, and C ABI registry shims while preserving subproc
  synchronization and arena ownership.
- `SAFETY[AUDIT:arena-slice-allocation-boundary]`:
  `src/allocator/arena.rs` owns arena slice allocation, arena commit/reuse,
  slice bitmap mutation, memid creation, and allocation metadata publication.
  Unsafe operations are limited to typed arena/range/layout witnesses,
  free/dirty/committed bitmaps, OS commit primitives, and stat adjustments
  while preserving zeroing and commit accounting.
- `SAFETY[AUDIT:arena-initialization-boundary]`:
  `src/allocator/arena.rs` owns arena metadata initialization, guard-page
  setup, bitmap layout, page metadata table setup, and initial registry
  insertion. Unsafe operations are limited to reserved arena memory, typed
  slice counts, bitmap initialization, and arena header writes while preserving
  reserved-range bounds and arena alignment.
- `SAFETY[AUDIT:arena-reserve-growth-boundary]`:
  `src/allocator/arena.rs` owns arena reserve growth and retry sizing.
  Unsafe operations are limited to subproc arena counts, reserve-lock guarded
  growth, virtual reserve feature checks, and stat adjustment balancing while
  preserving bounded reserve sizes and retry accounting.
- `SAFETY[AUDIT:arena-os-fallback-boundary]`:
  `src/allocator/arena.rs` owns OS allocation fallback for arena allocation
  requests and internal C-shaped arena allocation shims. Unsafe operations are
  limited to typed layout/alignment witnesses, errno slots, OS allocation
  primitives, and memid out-pointers while preserving disallow-options behavior
  and fallback semantics.
- `SAFETY[AUDIT:arena-public-reserve-boundary]`:
  `src/allocator/arena.rs` owns public reserve/manage arena APIs and internal
  OS-memory registration. Unsafe operations are limited to C ABI pointers,
  typed OS-memory and memid witnesses, arena initialization handoff, and OS
  release on registration failure while preserving alignment and caller-owned
  memory contracts.
- `SAFETY[AUDIT:arena-huge-reservation-boundary]`:
  `src/allocator/arena.rs` owns huge OS page reservation APIs. Unsafe
  operations are limited to NUMA normalization, huge-page OS allocation,
  memid publication, arena registration, and OS release on failure while
  preserving timeout, interleave, and reserved-page reporting semantics.
- `SAFETY[AUDIT:arena-abandoned-boundary]`: `src/allocator/arena.rs` owns
  abandoned-page ownership claim/release, heap arena-page bitmap lookup, and
  thread-local abandoned-page claim callbacks. Unsafe operations are limited to
  typed abandoned-page witnesses, remote-free collection, callback ABI
  arguments, and heap arena-page bitmap cells while preserving page ownership
  transfer and abandoned-count semantics.
- `SAFETY[AUDIT:arena-page-fresh-boundary]`:
  `src/allocator/arena.rs` owns fresh arena-backed page memory reservation,
  initial commit, page metadata placement, and page layout setup. Unsafe
  operations are limited to typed page/block/slice witnesses, arena memid
  projection, OS fallback handoff, and metadata initialization while preserving
  page alignment and reserved-block invariants.
- `SAFETY[AUDIT:arena-page-regular-boundary]`:
  `src/allocator/arena.rs` owns regular size-class page allocation and
  abandoned-page reuse during allocation. Unsafe operations are limited to
  typed slice-count/block-size witnesses, fresh-page initialization handoff,
  and page initialization failure cleanup while preserving size-class slice
  counts and page availability.
- `SAFETY[AUDIT:arena-page-singleton-boundary]`:
  `src/allocator/arena.rs` owns singleton page setup for over-aligned and
  large-object allocations. Unsafe operations are limited to block alignment,
  one-block reserved count checks, fresh-page setup, and failure cleanup while
  preserving singleton page invariants.
- `SAFETY[AUDIT:arena-page-free-boundary]`:
  `src/allocator/arena.rs` owns arena page free shims and internal free
  cleanup. Unsafe operations are limited to all-free page witnesses, arena
  memid projection, page-map unregister, bitmap clearing, stat adjustment, and
  backing-slice release while preserving ownership and abandoned-state checks.
- `SAFETY[AUDIT:arena-page-abandon-boundary]`:
  `src/allocator/arena.rs` owns arena page abandon shims and abandoned-state
  publication. Unsafe operations are limited to page-for-abandon witnesses,
  arena/heap abandoned bitmaps, owner-id words, heap lists, and stat updates
  while preserving mapped-abandoned visibility and thread ownership release.
- `SAFETY[AUDIT:arena-page-reabandon-unabandon-boundary]`:
  `src/allocator/arena.rs` owns reabandon and unabandon shims. Unsafe
  operations are limited to owned-abandoned page witnesses, mapped-abandoned
  bitmap transitions, heap abandoned lists, and stat updates while preserving
  same-thread ownership and count reconciliation.
- `SAFETY[AUDIT:arena-heap-page-publication-boundary]`:
  `src/allocator/arena.rs` owns heap arena-page bitmap publication during
  page allocation. Unsafe operations are limited to per-heap arena page tables,
  arena memid projection, bitmap set/rollback, and allocation cleanup while
  preserving visibility only after arena page metadata exists.
- `SAFETY[AUDIT:arena-page-ownership-transition-boundary]`:
  `src/allocator/arena.rs` owns arena page owner-word and page-map visibility
  transitions. Unsafe operations are limited to page metadata owner fields,
  thread-free ownership words, and page-map registration while preserving page
  metadata invariants before ownership becomes visible.
- `SAFETY[AUDIT:arena-purge-boundary]`: `src/allocator/arena.rs` owns arena
  purge scheduling, purge traversal callbacks, arena collection, arena free,
  and OS decommit/reset handoff. Unsafe operations are limited to purge bitmap
  traversal, elapsed-time decisions, typed arena/slice witnesses, OS purge
  primitives, and C ABI collection shims while preserving purge ordering,
  stats accounting, and lock discipline.
- `SAFETY[AUDIT:internal-bitmap-simd-boundary]`: `src/internal/bitmap.rs`
  owns optional AVX2 bitmap helper shims. Unsafe operations are limited to
  target-feature-gated intrinsic calls for zero/ones vector setup and bitmap
  all-set/all-clear probes.
- `SAFETY[AUDIT:internal-bitmap-metadata-boundary]`: `src/internal/bitmap.rs`
  owns bitmap and binned-bitmap sizing, initialization, and chunk-count
  metadata projection. Unsafe operations are limited to caller-proven bitmap
  storage, nullable chunk-count out-pointers, and zeroing initialized storage.
- `SAFETY[AUDIT:internal-bitmap-chunkmap-boundary]`: `src/internal/bitmap.rs`
  owns bitmap chunkmap and binned chunk-bin bookkeeping. Unsafe operations are
  limited to chunk-count assertions, empty/full chunk probes, and bin map
  updates that preserve chunkmap consistency.
- `SAFETY[AUDIT:internal-bitmap-claim-boundary]`: `src/internal/bitmap.rs`
  owns regular bitmap first-fit claim traversal and typed thread-local claim
  callback handoff. Unsafe operations are limited to caller-proven bitmap
  storage, callback data projection, and restoring bits when claim callbacks
  decline ownership.
- `SAFETY[AUDIT:internal-bitmap-range-chunk-boundary]`:
  `src/internal/bitmap.rs` owns regular bitmap set/clear/test/popcount
  operations over chunk ranges. Unsafe operations are limited to bounded
  chunk/field projection, caller-proven out-pointers, and release-mode
  clipping of out-of-range requests.
- `SAFETY[AUDIT:internal-bitmap-traversal-boundary]`:
  `src/internal/bitmap.rs` owns regular bitmap set/range traversal callbacks.
  Unsafe operations are limited to raw C callback invocation, typed arena and
  slice wrapper handoff, range formation, and rollback of unvisited bits.
- `SAFETY[AUDIT:internal-bitmap-binned-boundary]`: `src/internal/bitmap.rs`
  owns binned-bitmap set/clear/test and chunk-bin operations. Unsafe operations
  are limited to caller-proven binned-bitmap storage, bounded chunk projection,
  and bin metadata updates for small/medium/large/huge ranges.
- `SAFETY[AUDIT:internal-bitmap-binned-claim-boundary]`:
  `src/internal/bitmap.rs` owns binned-bitmap find-and-clear claim paths.
  Unsafe operations are limited to allocation-free first-fit search, cyclic
  chunk/bin scans, huge-range rollback, and caller-proven result out-pointers.
- `SAFETY[AUDIT:internal-bitmap-test-fixtures]`: `src/internal/bitmap.rs`
  owns local bitmap unit-test fixtures that pass stack-owned callback state
  through raw C callback arguments without escaping the call.
- `SAFETY[AUDIT:internal-runtime-shim-boundary]`: `src/internal/mod.rs` owns
  Rust-side assertion shims that forward static caller-proven C strings into the
  audited options abort path.
- `SAFETY[AUDIT:internal-c-abi-shim-boundary]`: `src/internal/mod.rs` owns the
  declaration-only internal C helper surface. The declarations preserve
  translated raw signatures without dereferencing pointers.
- `SAFETY[AUDIT:internal-cpu-memory-boundary]`: `src/internal/mod.rs` owns
  copy, fill, zero, and zero-check helpers. Unsafe operations are limited to
  caller-proven byte ranges, architecture-specific `rep movs/stos` thresholds,
  and fallback `ptr` copy/fill operations.
- `SAFETY[AUDIT:internal-thread-heap-shim-boundary]`: `src/internal/mod.rs`
  owns thread-heap and allocation helper projections. Unsafe operations load
  heap pointers, direct small-page slots, and guarded-allocation state from
  caller-proven thread-heap metadata through typed wrappers.
- `SAFETY[AUDIT:internal-direct-bin-boundary]`: `src/internal/mod.rs` owns
  direct-bin derivation from padded sizes. Unsafe operations are limited to
  unchecked `DirectBin` construction after page-direct table bounds checks.
- `SAFETY[AUDIT:internal-page-map-lookup-boundary]`: `src/internal/mod.rs`
  owns page-map lookup helpers. Unsafe operations convert allocator addresses
  through page-map indexes, submap pointers, and checked/unchecked page lookup.
- `SAFETY[AUDIT:internal-page-size-count-boundary]`: `src/internal/mod.rs`
  owns page size, count, committed-size, and availability projections from
  caller-proven page metadata into typed scalar domains.
- `SAFETY[AUDIT:internal-page-owner-heap-boundary]`: `src/internal/mod.rs`
  owns page heap, TLD, and queue ownership projections from caller-proven page
  or thread-heap metadata.
- `SAFETY[AUDIT:internal-page-location-boundary]`: `src/internal/mod.rs` owns
  page start, arena membership, slice start, slice offset, and address-range
  predicates derived from allocator-owned page metadata.
- `SAFETY[AUDIT:internal-page-flag-owner-boundary]`: `src/internal/mod.rs`
  owns page flags and owner-id projections. Unsafe operations use typed atomic
  thread-id fields to read and update abandoned/interior-pointer state.
- `SAFETY[AUDIT:internal-thread-free-boundary]`: `src/internal/mod.rs` owns
  thread-free ownership-word helpers. Unsafe operations are limited to typed
  atomic thread-free loads and ownership claims while preserving encoded head
  and ownership bits.
- `SAFETY[AUDIT:internal-block-link-boundary]`: `src/internal/mod.rs` owns
  encoded free-list block links. Unsafe operations preserve pointer provenance,
  tracking gates, page keys, encoded/unencoded link reads and writes, and
  same-page corruption checks.
- `SAFETY[AUDIT:free-raw-export-boundary]`: `src/allocator/free.rs` owns raw
  C ABI free, free-size, aligned-free, usable-size, and fast assembly alias
  shims. Unsafe operations are limited to symbol-preserving entry points and
  direct delegation into validated routing while preserving no-allocation
  interposition behavior.
- `SAFETY[AUDIT:free-validation-boundary]`: `src/allocator/free.rs` owns free
  pointer validation, double-free detection, interior/guarded pointer
  classification, and validation proof fixtures. Unsafe operations are limited
  to allocator-owned page/block/user-pointer witnesses, same-page checks, and
  guarded-block unprotection.
- `SAFETY[AUDIT:free-padding-boundary]`: `src/allocator/free.rs` owns padding
  decode, shrink, verification, and debug canary checks. Unsafe operations are
  limited to typed padding metadata on validated blocks and the matching
  tracking visibility transitions.
- `SAFETY[AUDIT:free-usable-size-boundary]`: `src/allocator/free.rs` owns
  usable-size queries for page/block, aligned/interior, and public user-pointer
  paths. Unsafe operations are limited to validated page metadata, unaligned
  block recovery, guarded-block adjustment, and caller-visible size shims.
- `SAFETY[AUDIT:free-stats-boundary]`: `src/allocator/free.rs` owns free-side
  statistic decrement updates. Unsafe operations are limited to current
  thread-heap counters and bin indexes derived from caller-validated pages.
- `SAFETY[AUDIT:free-local-list-boundary]`: `src/allocator/free.rs` owns local
  free-list mutation for blocks owned by the current thread heap. Unsafe
  operations are limited to typed local page/block witnesses, padding checks,
  local free-list pushes, used-count updates, retirement, and full-queue
  transitions.
- `SAFETY[AUDIT:free-remote-list-boundary]`: `src/allocator/free.rs` owns
  cross-thread free publication. Unsafe operations are limited to typed foreign
  page/block witnesses, thread-free atomic head updates, and abandoned-page
  collection handoff when ownership is not held.
- `SAFETY[AUDIT:free-route-dispatch-boundary]`: `src/allocator/free.rs` owns
  hot and cold free dispatch after validation. Unsafe operations are limited to
  validated free witnesses, local/remote/abandoned owner classification, usable
  out-pointers, and no-allocation routing.
- `SAFETY[AUDIT:free-abandoned-page-boundary]`: `src/allocator/free.rs` owns
  abandoned-page collection, reclaim, reabandon, and ownership release from the
  free path. Unsafe operations are limited to owned abandoned-page witnesses,
  remote free-list collection, arena suitability checks, and thread-free
  ownership-word release.
- `SAFETY[AUDIT:aligned-padding-boundary]`:
  `src/allocator/aligned.rs` owns padding shrink/grow behavior, over-allocation
  trimming, guarded aligned allocation, interior-pointer tagging, and tracking
  metadata for adjusted aligned blocks. Unsafe operations are limited to
  padding-shrink shims, page/block metadata, padding metadata writes, and
  tracking hooks while preserving usable-size and offset invariants.
- `SAFETY[AUDIT:aligned-thread-heap-allocation-boundary]`:
  `src/allocator/aligned.rs` owns thread-heap aligned malloc and zalloc entry
  paths after checked layout normalization. Unsafe operations are limited to
  local thread-heap witnesses and delegation into typed aligned allocation
  requests.
- `SAFETY[AUDIT:aligned-page-allocation-boundary]`:
  `src/allocator/aligned.rs` owns aligned allocation fast paths that inspect
  guarded allocation state and small-page free slots. Unsafe operations are
  limited to active page witnesses, usable-size out-pointers, direct page
  allocation, and tracking handoff for aligned blocks.
- `SAFETY[AUDIT:aligned-calloc-product-boundary]`:
  `src/allocator/aligned.rs` owns aligned calloc product handling for
  thread-heap and heap entry paths. Unsafe operations are limited to checked
  `CallocRequest` totals, heap/thread-heap witness creation, and zeroed aligned
  dispatch.
- `SAFETY[AUDIT:aligned-global-allocation-export-boundary]`:
  `src/allocator/aligned.rs` owns global aligned allocation C ABI exports.
  Unsafe operations are limited to raw global malloc/zalloc/calloc entry points
  forwarding into checked aligned request helpers.
- `SAFETY[AUDIT:aligned-heap-allocation-export-boundary]`:
  `src/allocator/aligned.rs` owns heap aligned allocation C ABI exports.
  Unsafe operations are limited to borrowing raw heap handles, deriving
  thread-heap witnesses, and forwarding checked aligned requests.
- `SAFETY[AUDIT:aligned-realloc-request-boundary]`:
  `src/allocator/aligned.rs` owns thread-heap aligned realloc request
  construction. Unsafe operations are limited to old user-pointer validation,
  checked layout creation, default/null allocation routing, and typed realloc
  request handoff.
- `SAFETY[AUDIT:aligned-realloc-copy-boundary]`:
  `src/allocator/aligned.rs` owns aligned realloc copy preservation. Unsafe
  operations are limited to newly allocated aligned blocks, old allocation
  usable-size metadata, bounded zeroing, bounded copy, and freeing the validated
  old block.
- `SAFETY[AUDIT:aligned-recalloc-product-boundary]`:
  `src/allocator/aligned.rs` owns aligned recalloc product handling. Unsafe
  operations are limited to checked `CallocRequest` totals, old allocation
  validation, zeroed layout construction, and typed recalloc handoff.
- `SAFETY[AUDIT:aligned-realloc-global-export-boundary]`:
  `src/allocator/aligned.rs` owns global aligned realloc C ABI exports. Unsafe
  operations are limited to raw realloc/rezalloc/recalloc entry points
  forwarding into checked default thread-heap requests.
- `SAFETY[AUDIT:aligned-realloc-heap-export-boundary]`:
  `src/allocator/aligned.rs` owns heap aligned realloc C ABI exports. Unsafe
  operations are limited to borrowing raw heap handles, validating old
  allocation pointers, checked layout/product handling, and forwarding typed
  heap realloc requests.
- `SAFETY[AUDIT:options-storage-lock-boundary]`: `src/allocator/options.rs`
  owns option descriptor storage and initialization. Unsafe operations are
  limited to mimalloc lock guards, atomic option values, bounded environment
  buffers, static option-name C strings, and raw option get/set exports.
- `SAFETY[AUDIT:options-libc-file-symbol-boundary]`:
  `src/allocator/options.rs` owns platform-gated C stdout/stderr symbol access
  for output callback compatibility checks.
- `SAFETY[AUDIT:options-recurse-guard-boundary]`:
  `src/allocator/options.rs` owns diagnostic output recursion guards. Unsafe
  operations are limited to TLS shim get/set calls before callback dispatch.
- `SAFETY[AUDIT:options-delayed-output-buffer-boundary]`:
  `src/allocator/options.rs` owns delayed diagnostic output buffering. Unsafe
  operations are limited to bounded static buffer access, mimalloc lock
  acquire/release, and flushing buffered bytes through the selected callback.
- `SAFETY[AUDIT:options-stderr-output-boundary]`:
  `src/allocator/options.rs` owns primitive stderr diagnostic forwarding. Unsafe
  operations are limited to validating C string pointers before calling the
  primitive backend.
- `SAFETY[AUDIT:options-output-callback-boundary]`:
  `src/allocator/options.rs` owns registered output callback lookup,
  publication, and invocation through typed callback wrappers.
- `SAFETY[AUDIT:options-format-output-boundary]`:
  `src/allocator/options.rs` owns bounded diagnostic formatting and
  thread-prefix formatting. Unsafe operations are limited to C string prefix
  length checks, stack-buffer conversion, and forwarding formatted bytes through
  the output callback path.
- `SAFETY[AUDIT:options-print-output-boundary]`:
  `src/allocator/options.rs` owns public option printing. Unsafe operations are
  limited to reading static option-name C strings and forwarding formatted option
  values through the output callback path.
- `SAFETY[AUDIT:options-warning-error-callback-boundary]`:
  `src/allocator/options.rs` owns warning display shims and warning/error
  counter gates. Unsafe operations are limited to C-compatible warning entry
  points and bounded message output.
- `SAFETY[AUDIT:options-allocation-failure-callback-boundary]`:
  `src/allocator/options.rs` owns registered error callback storage and
  allocation/error failure callback dispatch. Unsafe operations are limited to
  locked callback state, callback argument publication, callback invocation,
  and the default abort policy.
- `SAFETY[AUDIT:options-assertion-boundary]`: `src/allocator/options.rs` owns
  assertion failure abort shims. Unsafe operations read caller-proven C strings,
  call platform diagnostic output, preserve the test abort fixture, and abort
  without unwinding.
- `SAFETY[AUDIT:stats-counter-boundary]`: `src/allocator/stats.rs` owns stats
  initialization and counter/count mutation helpers. Unsafe operations are
  limited to POD zero initialization, checked bin indexes, and atomic counter
  field access while preserving counter width and stat-level gating.
- `SAFETY[AUDIT:stats-counter-merge-boundary]`: `src/allocator/stats.rs` owns
  raw stats counter merge bodies and reset handoff. Unsafe operations are
  limited to validated stats pointers, non-null counter merges, bin stat
  projection, and zeroing the source stats buffer after merge.
- `SAFETY[AUDIT:stats-clock-boundary]`: `src/allocator/stats.rs` owns stats
  clock reads. Unsafe operations delegate to the primitive clock source without
  allocation or retained state.
- `SAFETY[AUDIT:stats-heap-merge-boundary]`: `src/allocator/stats.rs` owns
  thread-heap to heap stats merging and heap stats lookup. Unsafe operations
  are limited to typed thread-heap/heap witnesses, initialized-thread checks,
  and borrowed heap stats pointers.
- `SAFETY[AUDIT:stats-public-merge-boundary]`: `src/allocator/stats.rs` owns
  public merge and reset exports. Unsafe operations preserve legacy process
  stats behavior, default heap lookup, and subproc merge handoff.
- `SAFETY[AUDIT:stats-heap-visitor-boundary]`: `src/allocator/stats.rs` owns
  heap stats print and aggregate visitors. Unsafe operations keep callback
  argument storage, borrowed heap witnesses, and stats buffers valid for the
  duration of heap walks.
- `SAFETY[AUDIT:stats-public-export-boundary]`: `src/allocator/stats.rs` owns
  public stats copy/get exports. Unsafe operations validate output buffers,
  subproc IDs, nullable heap pointers, and stats header versions before copying
  or aggregating results.
- `SAFETY[AUDIT:stats-output-boundary]`: `src/allocator/stats.rs` owns
  formatted stats output, stack-buffer amount formatting, callback forwarding,
  and raw print/export shims. Unsafe operations are limited to callback output
  buffers, legacy callback pointer transmutation, C string formatting, and C
  ABI print shims while preserving buffer bounds and allocator recursion
  constraints.
- `SAFETY[AUDIT:stats-process-info-boundary]`: `src/allocator/stats.rs` owns
  process-info collection and raw out-pointer publication. Unsafe operations
  are limited to platform process-info calls, nullable out-pointer writes, and
  elapsed-time reads while preserving C-compatible optional output behavior.
- `SAFETY[AUDIT:stats-json-boundary]`: `src/allocator/stats.rs` owns JSON
  output buffering, heap stats serialization, fixed stack formatting, and JSON
  callback routing. Unsafe operations are limited to JSON buffer allocation and
  free, C string buffer projection, callback writes, and heap stats witnesses
  while preserving buffer ownership and output bounds.
- `SAFETY[AUDIT:owned-allocation]`: `src/safe/allocation.rs` owns non-zero
  allocations through move-only wrappers. Slice views and copies are bounded by
  the stored layout, and `Drop` releases live blocks exactly once.
- `SAFETY[AUDIT:panic-boundary]`: `src/lib.rs` no-std panic handling and abort
  declarations terminate instead of unwinding across allocator or C ABI
  boundaries.
- `SAFETY[AUDIT:raw-auto-trait]`: `src/types/constants.rs` implements raw
  auto-traits only for immutable sentinel metadata values whose mutation remains
  protected by allocator ownership, atomics, or locks.
- `SAFETY[AUDIT:raw-layout-abi-typedefs]`: `src/types/constants.rs` owns raw
  C ABI callback typedef declarations for deferred-free, output, error, heap
  visit, block visit, managed-memory commit, arena claim, and forall-set
  callbacks. These declarations publish raw `unsafe extern "C" fn` signatures
  and contracts only; typed callback wrappers classify invocation and storage
  inside allocator internals.
- `SAFETY[AUDIT:raw-macro-boundary]`: `src/ffi/abi.rs` and
  `src/ffi/headers.rs` expose raw C-header macro compatibility without adding
  Rust ownership guarantees; callers inherit the documented raw ABI contract.
- `SAFETY[AUDIT:raw-macro-bootstrap-boundary]`: `src/raw/macros.rs` owns Unix
  bootstrap allocation tracking before allocator initialization. Unsafe
  operations are limited to bootstrap `mmap`/`munmap` declarations, header
  pointer reconstruction, sanitizer slot tracking, preloading handoffs, and
  fixtures that prove bootstrap ownership transfers to normal free/realloc.
- `SAFETY[AUDIT:raw-macro-override-entry-boundary]`: `src/raw/macros.rs` owns
  raw allocator override entry points, macOS interpose records, Windows CRT
  override exports, and fast assembly aliases. Unsafe operations are limited to
  exported ABI attributes, global/naked assembly, interpose records, foreign
  target declarations, and raw forwarding to allocator entry points.
- `SAFETY[AUDIT:raw-macro-unix-core-boundary]`: `src/raw/macros.rs` owns Unix
  malloc/calloc/realloc/free export shims. Unsafe operations are limited to
  exported C ABI wrappers that forward into the audited Unix bootstrap-aware
  helpers.
- `SAFETY[AUDIT:raw-macro-unix-string-boundary]`: `src/raw/macros.rs` owns Unix
  strdup/strndup forwarding shims. Unsafe operations are limited to raw
  nullable C string forwarding and exported libc symbol wrappers.
- `SAFETY[AUDIT:raw-macro-unix-realloc-boundary]`: `src/raw/macros.rs` owns
  Unix reallocf compatibility forwarding. Unsafe operations are limited to
  realloc failure cleanup through the same Unix free boundary.
- `SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]`:
  `src/raw/macros.rs` owns Unix valloc/vfree/pvalloc, memalign/aligned_alloc,
  cfree, `_aligned_malloc`, and POSIX memalign forwarding. Unsafe operations
  are limited to raw aligned allocation/free contracts and exported POSIX/libc
  symbol wrappers.
- `SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]`:
  `src/raw/macros.rs` owns Unix reallocarray/reallocarr forwarding. Unsafe
  operations are limited to product-size checked realloc forwarding and Linux
  weak alias publication.
- `SAFETY[AUDIT:raw-macro-wasi-libc-boundary]`: `src/raw/macros.rs` owns WASI
  libc malloc/calloc/realloc/free/memalign forwarding. Unsafe operations are
  limited to `__libc_*` export wrappers that preserve raw WASI libc ownership
  contracts.
- `SAFETY[AUDIT:raw-macro-linux-libc-boundary]`: `src/raw/macros.rs` owns
  Linux `__libc_*` and `__posix_memalign` forwarding shims. Unsafe operations
  are limited to Linux libc symbol wrappers that forward to the corresponding
  Unix or POSIX compatibility helpers.
- `SAFETY[AUDIT:raw-macro-allocation-query-boundary]`: `src/raw/macros.rs`
  owns malloc size, usable-size, and good-size compatibility queries. Unsafe
  operations are limited to checked heap-region probes and raw forwarding to
  allocator query exports, including bootstrap allocation size queries.
- `SAFETY[AUDIT:raw-macro-cxx-delete-boundary]`: `src/raw/macros.rs` owns
  scalar C++ delete compatibility symbols and sized operator helper shims.
  Unsafe operations are limited to forwarding Itanium delete symbols to the
  matching allocator free path.
- `SAFETY[AUDIT:raw-macro-cxx-array-delete-boundary]`: `src/raw/macros.rs` owns
  array C++ delete compatibility symbols and sized array helper shims. Unsafe
  operations are limited to forwarding Itanium array-delete symbols to the
  matching allocator free path.
- `SAFETY[AUDIT:raw-macro-cxx-aligned-boundary]`: `src/raw/macros.rs` owns
  aligned C++ new/delete compatibility symbols, including aligned nothrow and
  operator helper variants. Unsafe operations are limited to preserving
  `align_val_t` ABI signatures while forwarding to aligned allocator exports.
- `SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]`: `src/raw/macros.rs` owns
  non-aligned nothrow C++ compatibility shims. Unsafe operations are limited to
  consuming the nothrow tag ABI and forwarding through null-returning allocation
  paths or free paths.
- `SAFETY[AUDIT:raw-macro-cxx-new-boundary]`: `src/raw/macros.rs` owns scalar
  and array C++ new compatibility symbols for pointer-width-specific Itanium
  ABI variants. Unsafe operations are limited to forwarding to `mi_new`.
- `SAFETY[AUDIT:raw-macro-header-forwarding-boundary]`: `src/raw/macros.rs`
  owns header-shaped forwarding helpers used by raw macro compatibility layers.
  Unsafe operations are limited to calling caller-supplied raw C function
  pointers or forwarding to raw allocator exports without adding Rust ownership
  guarantees.
- `SAFETY[AUDIT:raw-posix-errno-boundary]`: `src/raw/posix.rs` owns platform
  errno publication. Unsafe operations are limited to thread-local errno slot
  lookup and writes on supported libc targets.
- `SAFETY[AUDIT:raw-posix-query-free-boundary]`: `src/raw/posix.rs` owns
  usable-size/malloc-size query aliases and `cfree`. Unsafe operations are
  limited to raw pointer query forwarding and heap-region-gated free.
- `SAFETY[AUDIT:raw-posix-aligned-allocation-boundary]`: `src/raw/posix.rs`
  owns POSIX and libc-aligned allocation exports. Unsafe operations are limited
  to alignment validation, nullable out-pointer writes, page-size rounding, and
  forwarding to mimalloc aligned allocation.
- `SAFETY[AUDIT:raw-posix-realloc-boundary]`: `src/raw/posix.rs` owns
  reallocarray, reallocarr, and expand compatibility. Unsafe operations are
  limited to checked product sizing, pointer-slot reads/writes, raw realloc
  forwarding, and POSIX errno publication.
- `SAFETY[AUDIT:raw-posix-string-dup-boundary]`: `src/raw/posix.rs` owns
  multibyte and wide-string duplication. Unsafe operations are limited to
  nullable C string forwarding, wide-string scanning, byte-size overflow
  checks, allocation, and bounded copy.
- `SAFETY[AUDIT:raw-posix-environment-boundary]`: `src/raw/posix.rs` owns
  environment lookup and duplication helpers. Unsafe operations are limited to
  process-owned environment pointers, validated nullable out-pointer writes,
  mimalloc-owned duplication, and unsupported wide-environment fallback state.
- `SAFETY[AUDIT:raw-posix-aligned-recalloc-boundary]`: `src/raw/posix.rs`
  owns Windows-compatible aligned recalloc forwarding. Unsafe operations are
  limited to raw aligned recalloc argument forwarding into the audited aligned
  allocation boundary.
- `SAFETY[AUDIT:unix-ffi-import-boundary]`: `src/platform/unix.rs` owns Unix
  primitive libc/syscall declarations and errno slot imports. Unsafe operations
  are limited to source-present FFI declarations and errno reads while keeping
  the primitive backend allocation-free and C ABI compatible.
- `SAFETY[AUDIT:unix-mmap-syscall-boundary]`: `src/platform/unix.rs` owns Unix
  `mmap`/`munmap`/`madvise`/`mprotect` allocation primitives and direct file or
  syscall helper wrappers. Unsafe operations are limited to caller-proven
  address ranges, file descriptors, alignment hints, errno propagation, and
  nullable out-pointer writes.
- `SAFETY[AUDIT:unix-process-info-boundary]`: `src/platform/unix.rs` owns Unix
  process, clock, page-size, overcommit, and physical-memory probes. Unsafe
  operations are limited to `sysconf`, proc/sys reads, `clock_gettime`,
  `getrusage`, and process-info writes.
- `SAFETY[AUDIT:unix-output-env-boundary]`: `src/platform/unix.rs` owns Unix
  stderr and environment helpers. Unsafe operations are limited to C-string
  reads, direct `write(2)` loops, `environ`/`getenv` traversal, case-folding
  fallback buffers, and bounded caller output copies.
- `SAFETY[AUDIT:unix-random-boundary]`: `src/platform/unix.rs` owns Unix random
  buffer filling. Unsafe operations are limited to platform entropy imports,
  `getrandom` syscall fallback, `/dev/urandom` open/read/close loops, and
  caller-provided buffer writes.
- `SAFETY[AUDIT:unix-thread-tls-boundary]`: `src/platform/unix.rs` owns Unix
  pthread destructor-key lifecycle, default thread-heap association, threadpool
  query fallback, and scheduler yield. Unsafe operations are limited to
  pthread key create/delete/set calls and thread-heap destructor callbacks.
- `SAFETY[AUDIT:unix-huge-page-boundary]`: `src/platform/unix.rs` owns Unix
  transparent huge-page detection and huge/large page allocation fallback.
  Unsafe operations are limited to THP proc/sys reads, `MAP_HUGETLB`/huge-size
  `mmap` attempts, large-page out-pointer writes, and fallback warnings.
- `SAFETY[AUDIT:unix-numa-boundary]`: `src/platform/unix.rs` owns Unix NUMA
  helpers. Unsafe operations are limited to `mbind`/`getcpu` syscalls, sysfs
  node-count probes, node-mask construction, and NUMA placement fallbacks.
- `SAFETY[AUDIT:windows-virtual-memory-boundary]`: `src/platform/windows.rs`
  owns Windows virtual-memory allocation, commit, decommit, reset, protect,
  query, aligned reservation, retry, and error-code paths. Unsafe operations
  are limited to `VirtualAlloc`/`VirtualFree`/`VirtualProtect`/`VirtualQuery`,
  `VirtualAlloc2`/`NtAllocateVirtualMemoryEx` lookup and calls, nullable
  out-pointer writes, and Windows error propagation.
- `SAFETY[AUDIT:windows-process-info-boundary]`: `src/platform/windows.rs`
  owns Windows process, time, version, system-info, and memory counter helpers.
  Unsafe operations are limited to `QueryPerformanceFrequency`, `FILETIME`
  projection, `GetSystemInfo`, `GetProcessTimes`, dynamic PSAPI lookup, and
  process-info out-pointer writes.
- `SAFETY[AUDIT:windows-output-env-boundary]`: `src/platform/windows.rs` owns
  Windows stderr and environment helpers. Unsafe operations are limited to
  C-string reads, case-insensitive environment lookup, bounded result copying,
  standard-error handle probing, console/file writes, and std fallback output.
- `SAFETY[AUDIT:windows-tls-fls-boundary]`: `src/platform/windows.rs` owns
  Windows TLS/FLS, DLL lifecycle, CRT attach/detach, allocator init/done shims,
  TEB thread-pool probing, and thread-yield helpers. Unsafe operations are
  limited to callback dispatch, FLS key create/delete/set calls, thread-heap
  destructor callbacks, and attach/detach forwarding.
- `SAFETY[AUDIT:windows-large-page-boundary]`: `src/platform/windows.rs` owns
  Windows large and huge page privilege enablement, page-size probing, large
  allocation attempts, huge-page fallback, and large-allocation out-pointers.
  Unsafe operations are limited to token privilege calls, dynamic large-page
  function calls, large-page atomic fallback state, and caller-provided result
  writes.
- `SAFETY[AUDIT:windows-numa-boundary]`: `src/platform/windows.rs` owns Windows
  NUMA helpers and NUMA hints used by huge allocations. Unsafe operations are
  limited to dynamic NUMA API calls, processor-number queries, node masks,
  highest-node probing, and NUMA extended-parameter setup.
- `SAFETY[AUDIT:windows-etw-boundary]`: `src/platform/windows.rs` owns the
  translated Windows ETW helper surface. Unsafe operations are limited to ETW
  provider registration/unregistration, enable callbacks, event descriptors,
  event-data pointer projection, bitmask mutation, and event write forwarding.
- `SAFETY[AUDIT:windows-random-boundary]`: `src/platform/windows.rs` owns
  Windows random primitive declarations and dynamic RNG function pointers.
  Unsafe operations are limited to ABI declarations for system RNG entrypoints
  and caller-provided random buffers.
- `SAFETY[AUDIT:windows-dynamic-import-boundary]`: `src/platform/windows.rs`
  owns the typed dynamic import helper. Unsafe operations are limited to
  `GetProcAddress` calls and copying non-null function pointers into requested
  function pointer types.
- `SAFETY[AUDIT:os-memory-config-boundary]`: `src/platform/os.rs` owns OS
  memory configuration, sub-process stats projection, page-size queries, and
  aligned address hints. Unsafe operations are limited to process-global memory
  config reads/writes, page-size statics, typed subproc witnesses, and primitive
  hint calls while preserving allocator-recursion avoidance.
- `SAFETY[AUDIT:os-memory-guard-boundary]`: `src/platform/os.rs` owns secure
  guard-page placement and reset callbacks. Unsafe operations are limited to
  random guard-page offset selection, primitive guard-page calls, and retained
  local helper shims while preserving page-size alignment and nullable pointer
  behavior.
- `SAFETY[AUDIT:os-memory-commit-boundary]`: `src/platform/os.rs` owns
  commit, decommit, reset, reuse, purge, protect, and unprotect state
  transitions. Unsafe operations are limited to typed OS pointer/range
  witnesses, memid zero/commit state updates, primitive range calls, and stats
  updates while preserving commit accounting.
- `SAFETY[AUDIT:os-memory-free-boundary]`: `src/platform/os.rs` owns OS memory
  free paths, huge-page free helpers, and retained free wrappers. Unsafe
  operations are limited to recorded memid witnesses, committed-byte
  classification, primitive free calls, and huge-page segment iteration while
  preserving pinned/committed semantics.
- `SAFETY[AUDIT:os-memory-primitive-allocation-boundary]`:
  `src/platform/os.rs` owns the primitive OS reserve/allocation handoff and
  reserved/committed stat accounting. Unsafe operations are limited to
  primitive allocation calls, caller-provided large/zero out-pointers, and
  subproc stat projection while preserving page-size alignment.
- `SAFETY[AUDIT:os-memory-aligned-allocation-boundary]`:
  `src/platform/os.rs` owns aligned OS allocation request shaping, over-reserve
  alignment, partial-free trimming, and offset adjustment. Unsafe operations are
  limited to typed size/page witnesses, aligned pointer arithmetic, direct
  primitive allocation dispatch, and aligned wrapper calls.
- `SAFETY[AUDIT:os-memory-memid-provenance-boundary]`: `src/platform/os.rs`
  owns raw memory-id initialization, OS/external memid projection, and memid
  publication to caller out-pointers. Unsafe operations are limited to writable
  memid slots and preserving recorded base/size/provenance flags.
- `SAFETY[AUDIT:os-memory-zeroing-boundary]`: `src/platform/os.rs` owns
  zero-allocation and post-allocation zeroing helpers. Unsafe operations are
  limited to committed writable ranges and memid zero/commit flag updates.
- `SAFETY[AUDIT:os-memory-request-allocation-boundary]`: `src/platform/os.rs`
  owns direct internal OS allocation entry points that produce raw OS memids.
  Unsafe operations are limited to good-size normalization, primitive
  allocation dispatch, and nullable memid output behavior.
- `SAFETY[AUDIT:os-memory-public-export-boundary]`: `src/platform/os.rs` owns
  retained local helper shims and public C ABI OS allocation wrappers. Unsafe
  operations are limited to nullable output pointers, wrapper forwarding, and
  exported symbol compatibility without taking ownership beyond the C contract.
- `SAFETY[AUDIT:os-memory-huge-page-boundary]`: `src/platform/os.rs` owns huge
  OS page detection, reservation, claiming, and aligned huge allocation.
  Unsafe operations are limited to primitive huge-page calls, arena claim
  callback handoff, per-segment memid state, and huge-page free rollback while
  preserving huge-page size and NUMA placement contracts.
- `SAFETY[AUDIT:os-memory-numa-boundary]`: `src/platform/os.rs` owns NUMA node
  count and current-node helper shims. Unsafe operations are limited to
  primitive NUMA calls and retained C ABI wrappers while preserving nullable
  and fallback behavior.
- `SAFETY[AUDIT:macos-zone-layout-boundary]`: `src/platform/macos.rs` owns the
  source-present Darwin malloc-zone struct, static mimalloc zone metadata, and
  malloc-zone import declarations. Unsafe operations are limited to raw C ABI
  callback slots and process-global zone metadata with stable layout.
- `SAFETY[AUDIT:macos-zone-lookup-boundary]`: `src/platform/macos.rs` owns
  zone creation/default-zone/name/all-zones lookup shims, jumpstart
  compatibility, pointer ownership queries, and usable-size forwarding.
- `SAFETY[AUDIT:macos-zone-allocation-boundary]`: `src/platform/macos.rs` owns
  malloc/calloc/valloc/free/realloc/memalign zone trampolines. Unsafe operations
  are limited to mimalloc ownership checks, direct mimalloc dispatch, and
  non-mimalloc callback forwarding with original pointers and sizes.
- `SAFETY[AUDIT:macos-zone-batch-boundary]`: `src/platform/macos.rs` owns batch
  malloc/free pointer-array traversal. Unsafe operations are limited to
  caller-provided pointer-slot reads/writes, scalar zone trampoline dispatch,
  and consumed-slot clearing.
- `SAFETY[AUDIT:macos-zone-fork-boundary]`: `src/platform/macos.rs` owns
  exported Darwin fork prepare/parent/child hooks and local no-op compatibility
  helpers.
- `SAFETY[AUDIT:macos-zone-compat-boundary]`: `src/platform/macos.rs` owns
  malloc-zone compatibility no-ops and destroy forwarding for check, log,
  print, pointer-info, register, unregister, printf, and non-mimalloc destroy
  callbacks.
- `SAFETY[AUDIT:macos-zone-interpose-boundary]`: `src/platform/macos.rs` owns
  the Darwin interpose section entries for malloc-zone replacement/target
  pairs.
- `SAFETY[AUDIT:macos-zone-init-boundary]`: `src/platform/macos.rs` owns macOS
  process initialization registration for the mimalloc zone, default-zone
  ordering, purgeable-zone ordering, and constructor-section publication.
- `SAFETY[AUDIT:platform-dynamic-tls-key-boundary]`: `src/platform/mod.rs` owns
  Apple fixed-slot TLS reads, dynamic pthread/Win32 key lookups, and expansion
  slot pointer selection without taking ownership of thread-heap metadata.
- `SAFETY[AUDIT:platform-linux-tls-shim-boundary]`: `src/platform/mod.rs` owns
  hidden Linux C TLS shim declarations, x86_64 initial-exec assembly loads and
  stores, thread-pointer reads, empty thread-heap symbol loads, and Linux TLS
  shim tests.
- `SAFETY[AUDIT:platform-thread-heap-association-boundary]`:
  `src/platform/mod.rs` owns default/cached thread-heap get/set helpers, empty
  sentinel fallbacks, heap/thread-heap association lookup, and page-associated
  thread-heap peeks.
- `SAFETY[AUDIT:platform-allocator-shim-boundary]`: `src/platform/mod.rs` owns
  redirected-query and allocator init/done C ABI compatibility shims, including
  fast-malloc assembly symbols and nullable diagnostic out-pointer writes.
- `SAFETY[AUDIT:internal-atomic-raw-view-boundary]`:
  `src/internal/atomic.rs` owns raw scalar storage reinterpretation as atomic
  views. Unsafe operations are limited to caller-proven
  `AtomicUsize`/`AtomicIsize`/`AtomicI64` shared references for the duration of
  one atomic operation.
- `SAFETY[AUDIT:internal-atomic-load-store-boundary]`:
  `src/internal/atomic.rs` owns scalar load/store helpers. Unsafe operations are
  limited to caller-proven scalar atomic storage and central memory-order
  mapping for `usize` and `i64` loads/stores.
- `SAFETY[AUDIT:internal-atomic-fetch-boundary]`:
  `src/internal/atomic.rs` owns scalar fetch arithmetic and bitwise helpers.
  Unsafe operations are limited to caller-proven scalar atomic storage,
  fetch-add/sub/and/or operations, and simple forwarding helpers.
- `SAFETY[AUDIT:internal-atomic-cas-boundary]`: `src/internal/atomic.rs` owns
  scalar compare-exchange helpers. Unsafe operations are limited to
  caller-proven atomic storage, caller-provided expected slots, memory-order
  mapping, and writing the observed value back on failed CAS.
- `SAFETY[AUDIT:internal-atomic-exchange-boundary]`:
  `src/internal/atomic.rs` owns scalar exchange helpers. Unsafe operations are
  limited to caller-proven scalar atomic storage and central memory-order
  mapping for swap operations.
- `SAFETY[AUDIT:internal-atomic-max-boundary]`: `src/internal/atomic.rs` owns
  the relaxed i64 maximum helper. Unsafe operations are limited to
  caller-proven i64 atomic storage and the compare-exchange loop that publishes
  a larger value.
- `SAFETY[AUDIT:internal-atomic-volatile-delta-boundary]`:
  `src/internal/atomic.rs` owns the void-add volatile addend helper. Unsafe
  operations are limited to reading the caller-provided volatile delta and
  applying it to caller-proven i64 atomic storage when nonzero.
- `SAFETY[AUDIT:internal-atomic-pointer-boundary]`: `src/internal/atomic.rs`
  owns pointer-sized atomic helpers. Unsafe operations are limited to
  pointer-to-`usize` atomic representation, pointer CAS expected-slot updates,
  pointer loads/stores/exchanges, and restoring raw pointer values unchanged.
- `SAFETY[AUDIT:internal-atomic-guard-once-boundary]`: `src/internal/atomic.rs`
  owns raw guard and once-state entry/release shims. Unsafe operations are
  limited to caller-proven guard/once storage, typed `AtomicGuard` and
  `OnceGuard` construction, completion publication, and macro entry forwarding.
- `SAFETY[AUDIT:internal-atomic-platform-lock-boundary]`:
  `src/internal/atomic.rs` owns platform lock compatibility shims. Unsafe
  operations are limited to Linux pthread mutex imports and calls, fallback
  spin-lock raw storage access, lock init/done behavior, and lock
  acquire/release forwarding.
- `SAFETY[AUDIT:page-map-global-boundary]`: `src/allocator/page_map.rs` owns
  process-global page-map metadata, base pointers, commit bitmaps, and typed
  page span/index projection. Unsafe operations are limited to allocator
  lifecycle synchronized global reads/writes and typed page metadata witnesses.
- `SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]`:
  `src/allocator/page_map.rs` owns 32-bit commit-bitmap driven page-map chunk
  commitment. Unsafe operations are limited to validated bitmap indexes,
  reserved byte-map offsets, OS commit calls, zeroing committed chunks, and
  bitmap publication.
- `SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]`:
  `src/allocator/page_map.rs` owns 32-bit page-map bootstrap, including virtual
  address sizing, backing reservation, commit bitmap allocation, memid state,
  and byte-map publication before concurrent use.
- `SAFETY[AUDIT:page-map-top-level-lazy-commit-boundary]`:
  `src/allocator/page_map.rs` owns non-32-bit lazy top-level table commit and
  submap-slot publication. Unsafe operations are limited to caller-provided
  empty out slots, commit-bit-derived table offsets, OS commit calls, and
  acquire/release commit-bit publication.
- `SAFETY[AUDIT:page-map-global-bootstrap-boundary]`:
  `src/allocator/page_map.rs` owns non-32-bit global page-map bootstrap,
  top-level table reservation, max-address/count globals, memid-derived commit
  state, and top-level base publication before concurrent use.
- `SAFETY[AUDIT:page-map-initial-submap-boundary]`:
  `src/allocator/page_map.rs` owns first-submap carving, commit, and zeroing
  during non-32-bit initialization. Unsafe operations are limited to pointer
  arithmetic inside the reserved backing allocation and OS commit/zero calls.
- `SAFETY[AUDIT:page-map-null-sentinel-boundary]`:
  `src/allocator/page_map.rs` owns null-pointer sentinel installation for both
  page-map layouts. Unsafe operations are limited to typed zero index/slice
  witnesses, committing or publishing index zero, lock initialization, and
  post-publication null lookup verification.
- `SAFETY[AUDIT:page-map-teardown-boundary]`: `src/allocator/page_map.rs` owns
  page-map unsafe destroy paths. Unsafe operations are limited to lifecycle
  exclusive global reads, recorded memid witnesses, submap iteration, and OS
  free calls while preserving teardown after all users are gone.
- `SAFETY[AUDIT:page-map-commit-boundary]`: `src/allocator/page_map.rs` owns
  page-map commit checks used by registration, unregistration, and range
  primitives. Unsafe operations are limited to caller-provided out slots and
  typed top-level index/count witnesses before reading or mutating page-map
  slots.
- `SAFETY[AUDIT:page-map-page-register-boundary]`:
  `src/allocator/page_map.rs` owns page registration entry points and typed
  page registration setup. Unsafe operations are limited to raw page validation,
  page-alignment checks, lazy initialization checks, page-map index derivation,
  and typed page witnesses.
- `SAFETY[AUDIT:page-map-range-register-boundary]`:
  `src/allocator/page_map.rs` owns registered page-range witness construction
  after range publication. Unsafe operations are limited to typed page-map
  index/sub-index/count witnesses, byte/submap range writes, rollback on
  failure, and `RegisteredPageRange` construction.
- `SAFETY[AUDIT:page-map-page-unregister-boundary]`:
  `src/allocator/page_map.rs` owns page unregistration entry points and typed
  page unregister setup. Unsafe operations are limited to raw page validation,
  initialized map checks, page-alignment checks, page-map index derivation, and
  registered-range witnesses.
- `SAFETY[AUDIT:page-map-range-unregister-boundary]`:
  `src/allocator/page_map.rs` owns registered range clearing and raw range
  unregister helpers. Unsafe operations are limited to typed index/sub-index
  and slice-count witnesses, C-compatible null-base no-op checks, and bounded
  byte/submap range clears.
- `SAFETY[AUDIT:page-map-submap-publication-boundary]`:
  `src/allocator/page_map.rs` owns lock-protected submap allocation and
  publication. Unsafe operations are limited to growth-lock acquisition,
  acquire loads of top-level slots, zeroed OS allocation, CAS publication, and
  freeing unneeded submaps after publication races.
- `SAFETY[AUDIT:page-map-range-primitive-boundary]`:
  `src/allocator/page_map.rs` owns non-32-bit range set/clear primitives.
  Unsafe operations are limited to typed top-level/sub-index witnesses, submap
  slot writes/clears, and bounded advancement across submap boundaries.
- `SAFETY[AUDIT:page-map-lookup-boundary]`: `src/allocator/page_map.rs` owns
  arbitrary-pointer page lookup and heap-region queries. Unsafe operations are
  limited to nullable input handling, committed index checks, submap reads, and
  page metadata projection while preserving safe lookup for invalid pointers.
- `SAFETY[AUDIT:scalar-domain-wrappers]`: `src/types/scalar.rs` owns transparent
  scalar domains for allocator sizes, alignments, indexes, bins, counts, and
  optional out-pointers. Unsafe operations are limited to unchecked non-zero
  constructors, caller-proven index/count/bin constructors, const
  `SliceCount` construction, nullable `UsableOut` writes, and lint-touch
  fixtures that use known-valid scalar values without publishing allocator
  state.
- `SAFETY[AUDIT:random-context-boundaries]`: `src/internal/random.rs`
  confines random context raw pointers and primitive entropy/clock calls to
  the allocator's C-compatible random boundary. The shims convert live context
  pointers to borrowed references once, keep ChaCha state updates in safe Rust,
  and use stack-local key buffers for primitive entropy reads.
- `SAFETY[AUDIT:wasi-memory-growth-boundary]`: `src/platform/wasi.rs` owns the
  WASI `sbrk` import, checked linear-memory growth, primitive memory
  initialization, primitive allocation, and no-op primitive free shim.
- `SAFETY[AUDIT:wasi-range-primitive-boundary]`: `src/platform/wasi.rs` owns
  commit/decommit/reset/reuse/protect no-op range shims, single-node NUMA
  queries, and unsupported huge-page result publication through nullable
  out-pointers.
- `SAFETY[AUDIT:wasi-io-query-boundary]`: `src/platform/wasi.rs` owns process
  info, stderr output, environment lookup imports and bounded C-string copying.
- `SAFETY[AUDIT:wasi-thread-shim-boundary]`: `src/platform/wasi.rs` owns thread
  init/done, default thread-heap association, threadpool, yield, and
  compatibility allocation helper shims that do not dereference raw thread-heap
  pointers.
- `SAFETY[AUDIT:emscripten-import-boundary]`:
  `src/platform/emscripten.rs` owns the Emscripten primitive import declarations
  and pthread destructor callback signature.
- `SAFETY[AUDIT:emscripten-memory-boundary]`:
  `src/platform/emscripten.rs` owns primitive memory initialization,
  emmalloc-backed allocation/free, nullable allocation out-pointers,
  commit/decommit/reset/reuse/protect shims, and unsupported huge-page results.
- `SAFETY[AUDIT:emscripten-clock-io-boundary]`:
  `src/platform/emscripten.rs` owns NUMA constants, clock queries, process-info
  fallback, console/stderr output, and unsupported environment lookup shims.
- `SAFETY[AUDIT:emscripten-pthread-key-boundary]`:
  `src/platform/emscripten.rs` owns pthread default-heap key lifecycle,
  destructor forwarding, static key mutation, teardown, and thread-local heap
  association.
- `SAFETY[AUDIT:emscripten-thread-shim-boundary]`:
  `src/platform/emscripten.rs` owns threadpool and scheduler-yield compatibility
  shims.
- `SAFETY[AUDIT:arena-debug-render]`: `src/allocator/arena/debug.rs` renders
  arena diagnostics by reading arena/page metadata discovered through the
  current sub-process registry, page map lookup, and typed memid views. The
  helpers are diagnostic-only, keep output in stack-local byte buffers, and do
  not allocate, publish, or mutate allocator ownership state.
- `SAFETY[AUDIT:arena-meta-allocator]`: `src/allocator/arena_meta.rs` owns
  arena metadata block allocation and free. Unsafe operations are limited to
  checked meta block-count construction, typed `MemId::<MetaMem>` packaging,
  meta-page pointer arithmetic, bitmap allocation/free updates, acquire/release
  meta-page stack publication and traversal, arena/OS fallback calls, and debug
  meta-page classification.
- `SAFETY[AUDIT:bench-diagnostics-ffi]`:
  `src/allocator/bench_diagnostics.rs` owns benchmark-only C ABI diagnostic
  counters and JSON export helpers. Unsafe operations are limited to exported
  diagnostic symbols, nullable snapshot out-pointer validation, bounded JSON
  buffer copies, NUL termination for non-empty buffers, and tests that call the
  same C-shaped boundary with valid local storage. Counter updates stay relaxed
  atomics, and the JSON writer remains allocation-free.
- `SAFETY[AUDIT:arena-id-memid-helpers]`:
  `src/allocator/arena/ids.rs` keeps raw arena-id ABI shims and memid
  classification behind typed `ArenaPtr`, `SliceRange`, `MemIdView`, and
  `PagePtr` witnesses. Unsafe operations are limited to arena prefix reads,
  validated out-pointer writes, and page/base pointer offset checks for
  allocator-owned memid ranges.
- `SAFETY[AUDIT:arena-heap-visit-callback-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns raw bitmap callback adaptation into
  typed heap/page visit state.
- `SAFETY[AUDIT:arena-heap-block-visit-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns public and internal block-visit C
  ABI wrappers and non-null heap witness construction.
- `SAFETY[AUDIT:arena-heap-page-iteration-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns heap arena-page bitmap iteration for
  visit callbacks.
- `SAFETY[AUDIT:arena-heap-abandoned-visit-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns abandoned-page visit wrappers and
  lock-protected abandoned list traversal.
- `SAFETY[AUDIT:arena-heap-delete-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns delete callback page claiming,
  unlinking, empty-page release, and non-empty page destruction setup.
- `SAFETY[AUDIT:arena-heap-transfer-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns heap-to-heap page transfer, including
  source/target arena-page bitmaps and stat/abandon publication.
- `SAFETY[AUDIT:arena-heap-delete-dispatch-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns heap delete traversal dispatch and
  debug-only post-drain verification.
- `SAFETY[AUDIT:arena-heap-move-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns page move C ABI shims and typed
  source/target heap witness conversion.
- `SAFETY[AUDIT:arena-heap-destroy-boundary]`:
  `src/allocator/arena/heap_pages.rs` owns non-main heap page destruction C ABI
  shims and owned heap destruction handoff.
- `SAFETY[AUDIT:arena-prefix-accessors]`:
  `src/allocator/arena/prefix.rs` centralizes heap, arena, sub-process, and
  arena-page prefix accessors. Callers supply allocator-owned metadata plus
  typed `BorrowedHeap`, `SubprocPtr`, `ArenaIndex`, or `Bin` witnesses before
  the helpers return typed atomic field wrappers or registered arena pointers.
- `SAFETY[AUDIT:callback-wrappers]`: `src/types/callbacks.rs` keeps internal
  callback function pointers and context pointers in `repr(transparent)`
  wrappers. Unsafe operations are limited to restoring callback pointers from
  allocator-owned storage and invoking caller-provided C ABI callbacks after
  typed argument conversion, with no allocation or dynamic dispatch.
- `SAFETY[AUDIT:deferred-free-callback-state]`:
  `src/allocator/page/deferred.rs` owns the C-compatible deferred-free callback
  globals, publishes callback and argument with release stores, reads them with
  acquire loads, and invokes callbacks only after bumping the local thread heap
  heartbeat and setting the per-thread-heap reentry guard.
- `SAFETY[AUDIT:guarded-config]`: `src/internal/guarded.rs` constructs guarded
  allocation sampling configuration from raw option values by clamping
  `size_max` to at least `size_min` before using the private unchecked
  constructor. Unit tests cover inverted bounds, sampling decisions, disabled
  sampling, and exposed typed fields.
- `SAFETY[AUDIT:page-count-accessors]`: `src/allocator/page/counts.rs`
  centralizes page and page-queue count field access. Callers supply live
  allocator-owned page or queue metadata, and the helpers route raw metadata
  through `PagePtr` and `PageQueuePtr` before typed counter arithmetic or
  publication.
- `SAFETY[AUDIT:page-flag-owner-accessors]`:
  `src/allocator/page/flags.rs` centralizes page flag, huge-page, full-queue,
  abandoned-page, and thread-heap owner access. Callers supply allocator-owned
  page metadata, and the helpers route raw fields through `PagePtr`,
  `PageFlags`, `MemId`, `AtomicPageThreadId`, and typed owner witnesses before
  reading or publishing page owner state.
- `SAFETY[AUDIT:page-validation-helpers]`:
  `src/allocator/page/validation.rs` centralizes page block lookup,
  free-list counting, free-list bounds checks, initialized-page validation, and
  owner queue membership checks. Callers supply allocator-owned page metadata,
  and the helpers wrap raw page pointers in typed witnesses before checking
  page/block list invariants.
- `SAFETY[AUDIT:page-bin-size-classes]`: `src/allocator/page/bins.rs`
  centralizes the bin/good-size table boundary, bounds unchecked `Bin`
  construction by `MI_BIN_COUNT`, and preserves the exported `mi_good_size`
  symbol while immediately routing into typed size domains.
- `SAFETY[AUDIT:safe-heap]`: `src/safe/heap.rs` ties heap allocations to the
  owning heap lifetime, prevents main-heap deletion through owned heap handles,
  and keeps raw heap/block calls behind wrapper ownership checks.
- `SAFETY[AUDIT:safe-process-info]`: `src/safe/stats.rs` passes valid local
  out-pointers to process-info collection and copies returned counters into
  typed Rust values before exposing them.
- `SAFETY[AUDIT:safe-stats]`: `src/safe/stats.rs` initializes stats buffers
  before use, passes valid writable output slices to JSON formatting, and
  exposes only typed totals/counts to callers.
- `SAFETY[AUDIT:threadlocal-empty-sentinel-boundary]`:
  `src/allocator/threadlocal.rs` owns the static empty dynamic TLS block.
- `SAFETY[AUDIT:threadlocal-tls-shim-boundary]`:
  `src/allocator/threadlocal.rs` owns Linux dynamic TLS shim access and x86_64
  initial-exec assembly for dynamic thread-local storage.
- `SAFETY[AUDIT:threadlocal-slot-block-boundary]`:
  `src/allocator/threadlocal.rs` owns current-thread slot block growth,
  copying, publication, and reset.
- `SAFETY[AUDIT:threadlocal-claim-callback-boundary]`:
  `src/allocator/threadlocal.rs` owns bitmap claim callback keep-set writes and
  callback witness restoration.
- `SAFETY[AUDIT:threadlocal-key-claim-boundary]`:
  `src/allocator/threadlocal.rs` owns versioned dynamic key claim routing.
- `SAFETY[AUDIT:threadlocal-free-bitmap-boundary]`:
  `src/allocator/threadlocal.rs` owns global dynamic key bitmap allocation,
  expansion, and release publication.
- `SAFETY[AUDIT:threadlocal-global-manager-boundary]`:
  `src/allocator/threadlocal.rs` owns global thread-local manager
  initialization and teardown.
- `SAFETY[AUDIT:threadlocal-key-access-boundary]`:
  `src/allocator/threadlocal.rs` owns dynamic key get/set slot access.
- `SAFETY[AUDIT:threadlocal-thread-done-boundary]`:
  `src/allocator/threadlocal.rs` owns per-thread dynamic TLS block teardown.
- `SAFETY[AUDIT:threadlocal-key-create-boundary]`:
  `src/allocator/threadlocal.rs` owns dynamic key creation.
- `SAFETY[AUDIT:threadlocal-key-free-boundary]`:
  `src/allocator/threadlocal.rs` owns dynamic key deletion and slot return.
- `SAFETY[AUDIT:tracking-hooks]`: `src/allocator/tracking.rs` owns the
  Valgrind/ASan tracking hook boundary. Default builds keep the hooks as
  allocation-free no-ops or assertions, while tracking feature builds forward
  caller-provided allocation ranges to the C-compatible shim signatures checked
  by the tracking parity scripts.
- `SAFETY[AUDIT:typed-freelist]`: `src/types/freelist.rs` reconstructs
  non-null block pointers from packed thread-free words after masking the
  ownership bit. Miri provenance helpers retain the original pointer identity,
  and `BlockPtr` adds only the non-null wrapper invariant.
- `SAFETY[AUDIT:provenance-miri-registry]`: `src/internal/provenance.rs`
  permits the Miri-only provenance registry wrapper to cross `RwLock`
  synchronization boundaries. The stored raw pointer values are lookup tokens
  for later reconstruction and do not grant aliasing or lifetime permissions.
- `SAFETY[AUDIT:typed-arena-domain]`: `src/types/arena.rs` owns typed
  slice-range construction and arena role witnesses. Unsafe operations are
  limited to reading initialized arena metadata, constructing non-zero
  `SliceCount` values after checked bounds, and packaging shared, exclusive, or
  parent arena role proofs into transparent wrappers.
- `SAFETY[AUDIT:typed-atomic-usize-field]`: `src/types/atomic.rs` owns
  temporary `AtomicUsize` views over caller-proven raw `usize` fields.
- `SAFETY[AUDIT:typed-atomic-i64-field]`: `src/types/atomic.rs` owns
  temporary `AtomicI64` views over caller-proven raw `i64` fields.
- `SAFETY[AUDIT:typed-atomic-pointer-field]`: `src/types/atomic.rs` owns
  temporary `AtomicPtr` views over caller-proven raw pointer fields.
- `SAFETY[AUDIT:typed-atomic-page-thread-id]`: `src/types/atomic.rs` owns typed
  page `xthread_id` load, flag, owner-store, and owner-CAS protocols.
- `SAFETY[AUDIT:typed-atomic-thread-free]`: `src/types/atomic.rs` owns typed
  `xthread_free` remote-list head and ownership bit protocols, including
  remote-list collection handoff fixtures.
- `SAFETY[AUDIT:typed-atomic-abandoned-count]`: `src/types/atomic.rs` owns
  abandoned-count atomic load/increment/decrement helpers.
- `SAFETY[AUDIT:typed-atomic-thread-heap-refcount]`: `src/types/atomic.rs` owns
  thread-heap refcount store/increment/decrement helpers.
- `SAFETY[AUDIT:typed-atomic-arena-page]`: `src/types/atomic.rs` owns arena
  page-table pointer atomic load/store helpers.
- `SAFETY[AUDIT:typed-atomic-guard-once-lock]`: `src/types/atomic.rs` owns
  RAII guard wrappers for atomic guards, once state, and mimalloc locks.
- `SAFETY[AUDIT:typed-arena-test-fixtures]`: `src/types/arena/tests.rs`
  builds stack-local arena metadata to exercise slice-range and arena-role
  constructors. The fixtures do not register with allocator globals and are
  used only while the stack-local records are live.
- `SAFETY[AUDIT:typed-atomic-test-fixtures]`: `src/types/atomic/tests.rs`
  builds stack-local atomic words, pointer cells, page/free-list metadata, once
  state, and lock storage to exercise typed atomic wrappers. The fixtures use
  atomic operations only, do not register with allocator globals, and remain
  live for the duration of each wrapper call.
- `SAFETY[AUDIT:typed-callback-test-fixtures]`: `src/types/callbacks/tests.rs`
  verifies callback wrapper layout, raw argument preservation, and storage
  pointer round trips using local callback function items and stack values.
- `SAFETY[AUDIT:typed-freelist-test-fixtures]`:
  `src/types/freelist/tests.rs` builds stack-local block metadata and opaque
  pointer words to exercise `ThreadFreeWord`, encoded-link, and raw link helper
  behavior. The fixtures are not registered with allocator globals.
- `SAFETY[AUDIT:typed-freelist-wrapper-test-fixtures]`:
  `src/types/freelist_wrappers/tests.rs` builds stack-local page and block
  metadata to exercise encoded pointers, block tags, available-list pop, and
  local-list push witnesses. The fixtures are not registered with allocator
  globals and are used only while the local records are live.
- `SAFETY[AUDIT:typed-freelist-wrappers]`:
  `src/types/freelist_wrappers.rs` owns typed encoded block links, block tags,
  available/local free-list heads, and allocated/free block witness transitions.
  Unsafe operations are limited to raw block-link reads and writes, page-local
  encoded pointer helpers, non-null block witness construction after null
  checks, and free-list pop/push conversions that preserve upstream layout.
- `SAFETY[AUDIT:typed-main-heap-test-fixtures]`:
  `src/types/heap/tests.rs` exercises main-heap witness construction from the
  allocator-owned process heap and verifies that main heaps cannot become owned
  deletion witnesses.
- `SAFETY[AUDIT:typed-owned-heap-test-fixtures]`:
  `src/types/heap/tests.rs` allocates real non-main heaps to exercise owned-heap
  witness acceptance and main-heap rejection. Each real heap is released exactly
  once before the test returns.
- `SAFETY[AUDIT:typed-borrowed-heap-test-fixtures]`:
  `src/types/heap/tests.rs` builds stack-local heap identity records and real
  heap lookup/allocation pointers to exercise borrowed-heap constructors, null
  rejection, and pointer preservation without transferring free ownership.
- `SAFETY[AUDIT:typed-thread-heap-state-test-fixtures]`:
  `src/types/heap/tests.rs` builds stack-local thread-heap records to exercise
  initialized, uninitialized, deleted, main, and borrowed thread-heap state
  classification. Stack-local records are used only while their fixtures are
  live.
- `SAFETY[AUDIT:typed-thread-local-heap-test-fixtures]`:
  `src/types/heap/tests.rs` builds stack-local TLD and thread-heap metadata to
  exercise local-thread promotion success and rejection. Real non-main heaps are
  released exactly once after the thread-local fixture is consumed.
- `SAFETY[AUDIT:typed-heap-domain]`: `src/types/heap.rs` owns heap,
  thread-heap, and subproc witness construction. Unsafe operations are limited
  to classifying initialized heap metadata, projecting heap and TLD pointers
  from valid thread-heap records, promoting initialized thread heaps to local
  witnesses after thread-id checks, and packaging transparent heap/subproc
  role witnesses without allocation.
- `SAFETY[AUDIT:typed-layout-domain]`: `src/types/layout.rs` owns allocation
  layout, product, realloc, and packed generic-flag request construction.
  Unsafe operations are limited to unchecked scalar constructors after local
  validation, checked multiplication handoff, validated old-allocation
  packaging, and raw flag-word wrapping without allocation.
- `SAFETY[AUDIT:typed-layout-test-fixtures]`: `src/types/layout/tests.rs` and
  the `src/types/layout.rs` lint-touch helper use concrete valid unchecked
  scalar values and non-dereferenced dangling pointer witnesses to exercise
  layout, realloc, and flag packing invariants.
- `SAFETY[AUDIT:typed-memid-view]`: `src/types/memid.rs` owns typed
  interpretation of the raw `mi_memid_t` union. Unsafe operations are limited
  to reading the `os`, `meta`, or `arena` union arm after matching the
  C-compatible `memkind` discriminant; null pointers, invalid meta block
  counts, and invalid arena slice ranges are rejected as `MemIdView::Unknown`.
- `SAFETY[AUDIT:typed-memid-test-fixtures]`: `src/types/memid/tests.rs` and
  the `src/types/memid.rs` lint-touch helper build stack-local or
  non-dereferenced OS, meta, arena, and raw memid fixtures to exercise typed
  memid constructors and unknown-view rejection. The fixtures are not
  published to allocator globals or exposed through wider helper visibility.
- `SAFETY[AUDIT:typed-padding-metadata]`: `src/types/padding.rs` confines raw
  padding metadata pointer construction, reads, and writes to `PaddingPtr`.
  Callers supply block pointers whose usable-size boundary is followed by live
  padding metadata, and raw canary/delta fields are converted through typed
  scalar wrappers.
- `SAFETY[AUDIT:typed-padding-test-fixtures]`: `src/types/padding/tests.rs`
  builds stack-local padding records and byte buffers, plus non-dereferenced
  dangling pointer witnesses for canary hashing, to exercise padding wrapper
  reads, writes, and pointer arithmetic.
- `SAFETY[AUDIT:typed-page-domain]`: `src/types/page.rs` owns page flag,
  tagged thread-id, free-path classification, and registered page-range witness
  construction. Unsafe operations are limited to unchecked constructors whose
  callers prove non-zero/thread-flag or page-map registration invariants before
  packaging those raw domains into transparent typed wrappers.
- `SAFETY[AUDIT:typed-pointer-constructor-boundary]`: `src/types/ptr.rs` owns
  transparent `NonNull` pointer wrapper constructors, const page pointers,
  user/interior pointer wrappers, page/slice starts, and OS memory pointers.
  Unsafe operations are limited to caller-proven non-null construction and
  non-dereferenced lint-touch references while preserving transparent layout
  and pointer-sized option niches.
- `SAFETY[AUDIT:typed-page-accessors]`: `src/types/ptr.rs` owns `PagePtr`
  metadata accessors. Unsafe operations are limited to page field reads/writes,
  free-list heads, atomic page ownership fields, page keys, block-area starts,
  and locally asserted scalar narrowing while preserving initialized page
  metadata invariants.
- `SAFETY[AUDIT:typed-block-accessors]`: `src/types/ptr.rs` owns `BlockPtr`
  free-list link accessors. Unsafe operations are limited to encoded link
  reads/writes and page-key based link decoding/encoding while preserving
  free-list provenance and page ownership requirements.
- `SAFETY[AUDIT:typed-heap-accessors]`: `src/types/ptr.rs` owns `HeapPtr`
  accessors. Unsafe operations are limited to heap subproc/thread-local fields,
  thread-heap links, exclusive arena state, NUMA state, arena-page cells, locks,
  and stats pointers while preserving heap metadata ownership.
- `SAFETY[AUDIT:typed-thread-heap-accessors]`: `src/types/ptr.rs` owns
  `ThreadHeapPtr` accessors. Unsafe operations are limited to TLD association,
  heap cells, refcount cells, heartbeat, page queue/direct-page fields, direct
  page array initialization, and stats pointers while preserving thread-heap
  locality requirements.
- `SAFETY[AUDIT:typed-tld-accessors]`: `src/types/ptr.rs` owns `TldPtr`
  accessors. Unsafe operations are limited to thread identity, sequence, NUMA,
  subproc, stats, arena table, memid, thread-heap, recursion, and thread-pool
  fields while preserving TLD ownership and optional subproc behavior.
- `SAFETY[AUDIT:typed-arena-accessors]`: `src/types/ptr.rs` owns `ArenaPtr`
  accessors. Unsafe operations are limited to arena memid, subproc, index,
  slice count, NUMA/exclusive flags, purge state, parent links, slice bitmaps,
  page metadata, and arena-page records while preserving arena table domains.
- `SAFETY[AUDIT:typed-subproc-accessors]`: `src/types/ptr.rs` owns
  `SubprocPtr` accessors. Unsafe operations are limited to sequence, arena
  counters, arena table entries, reserve/heaps locks, main heap/heaps links,
  thread and heap counters, memid, and stats pointer projection while
  preserving atomic ordering wrappers.
- `SAFETY[AUDIT:typed-page-queue-accessors]`: `src/types/ptr.rs` owns
  `PageQueuePtr` accessors. Unsafe operations are limited to queue first/last
  page links, queue counts, optional block-size domains, and lint-touch
  function references while preserving queue sentinel and block-size
  invariants.
- `SAFETY[AUDIT:typed-page-pointer-test-fixtures]`:
  `src/types/ptr/tests.rs` builds stack-local page metadata to exercise
  `PagePtr` field, free-list head, atomic ownership, remote-free, and key
  accessors without registering the page with allocator globals.
- `SAFETY[AUDIT:typed-block-pointer-test-fixtures]`:
  `src/types/ptr/tests.rs` builds stack-local block and page-key fixtures to
  exercise `BlockPtr` encoded-link read, write, clear, and next-pointer helper
  behavior.
- `SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]`:
  `src/types/ptr/tests.rs` builds stack-local heap, thread-heap, TLD, page,
  arena, and subproc records to exercise `HeapPtr`, `ThreadHeapPtr`, and
  `TldPtr` metadata fields while all stored pointers remain live.
- `SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]`:
  `src/types/ptr/tests.rs` builds stack-local arena, subproc, heap, page, and
  bitmap records to exercise arena/subproc pointer fields, counters, locks,
  memid, stats, and table accessors.
- `SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]`:
  `src/types/ptr/tests.rs` builds stack-local page-queue and page records to
  exercise first/last page links, counts, optional block sizes, and clearing
  behavior.
- `SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]`:
  `src/types/ptr/tests.rs` uses stack-local byte buffers plus non-dereferenced
  extreme or dangling pointer witnesses to exercise checked page/slice/OS
  pointer offsets and exposed-address provenance helpers.
- `SAFETY[AUDIT:typed-scalar-test-fixtures]`:
  `src/types/scalar/tests.rs` uses concrete raw scalar values to exercise
  unchecked constructor boundaries, overflow reporting, and table-index
  domains. The fixtures never allocate, touch allocator metadata, or escape
  the assertion that constructs them.
- `SAFETY[AUDIT:typed-page-test-fixtures]`: `src/types/page/tests.rs` builds
  stack-local page metadata and synthetic page-map range witnesses to exercise
  typed page wrapper accessors. The fixtures are not registered with allocator
  globals and are used only for local invariant tests.
- `SAFETY[AUDIT:typed-page-queue-test-fixtures]`:
  `src/types/page_queues/tests.rs` builds stack-local page and page-queue
  metadata to exercise normal, huge, full, linked, unlinked, and mismatched
  queue/page witnesses. The fixtures are not registered with allocator globals
  and are used only while the local records are live.
- `SAFETY[AUDIT:typed-page-queue-domain]`: `src/types/page_queues.rs` owns
  typed page-queue witness construction and transitions. Unsafe operations are
  limited to reading initialized queue metadata, delegating membership and
  full-state checks to page helpers, moving pages between already-typed queues,
  removing typed queued pages, and converting all-free local pages to destroyed
  arena-free witnesses.
- `SAFETY[AUDIT:typed-page-state-domain]`: `src/types/page_states.rs` owns
  page typestate witness construction and transitions. Unsafe operations are
  limited to transparent page and thread-heap witness construction, page-local
  free-list availability and pop state updates, all-free/full/retired/destroyed
  transitions, abandoned ownership claiming, remote-free classification, and
  thread-id based local, foreign, abandoned, and abandoned-mapped free-path
  classification.
- `SAFETY[AUDIT:typed-page-state-test-fixtures]`:
  `src/types/page_states.rs` and `src/types/page_states/tests.rs` build
  stack-local page metadata, non-dereferenced page identities, and
  non-dereferenced thread-heap identity witnesses to exercise page typestate
  transitions, ownership classification, lint-touch reachability, and
  abandoned-page claiming. The fixtures are not registered with allocator
  globals and are used only while the local records are live or as
  non-dereferenced identity values.
- `SAFETY[AUDIT:typed-free-validation]`: `src/types/validation.rs` converts
  validated pointer-lookup results into free-list witnesses, and
  `src/types/validation/tests.rs` builds stack-local page/block/user fixtures
  to exercise that conversion. The production conversion is reachable only
  after pointer lookup has associated the user pointer, page, and block, or
  through the explicitly unsafe fast path that requires the same proof from its
  caller.
- `SAFETY[AUDIT:unsafe-callback]`: `src/ffi/callbacks.rs` owns the unsafe
  callback registration/trampoline boundary. The registered argument pointer is
  created from the borrowed closure held by the guard, remains valid until
  `Drop` restores the previous process-global callback state, and trampoline
  entry catches Rust panics before returning through the C ABI where `std` or
  tests permit panic containment.

Evidence:

- `docs/unsafe-audit-evidence.tsv` maps each source `SAFETY[AUDIT:*]` operation
  ID to direct tests, validation scripts, structural audit checks, or manual
  proof. `scripts/check-unsafe-audit-evidence.py` rejects source IDs without
  evidence and evidence rows that reference missing files.

Migration:

- Operation-level enforcement now scans every Rust source file under `src/`.
  `scripts/check-unsafe-audit.py` verifies every source
  `SAFETY[AUDIT:<id>]` is listed exactly once here and rejects untagged unsafe
  blocks, unsafe impls, unsafe extern blocks, unsafe attributes, and assembly
  macro boundaries unless the owning file appears in
  `docs/unsafe-migration-allowlist.tsv`.
- The migration allowlist is intentionally file-level and temporary. Each entry
  must include a `production`, `test`, or `translation` category, an owner, a
  concrete `remove_by` date, and a reason. The checker caps the current
  allowlist at the current entry count; removing entries requires lowering that
  cap in the same change. Migrating a file means adding stable operation IDs
  near unsafe operations, documenting each ID in this section, and removing the
  file from the allowlist.

Validation:

- `scripts/check-unsafe-audit.py` verifies every `src/` file containing `unsafe`
  is listed here, checks operation IDs, enforces the dated migration allowlist,
  and verifies that the audit references the related atomic-ordering and
  tracked-mode follow-up audits.
- `scripts/check-unsafe-audit-evidence.py` verifies operation-level audit
  evidence for every source `SAFETY[AUDIT:*]` tag.
- `cargo test` exercises the public ABI, heap/theap/subprocess lifecycle,
  cross-thread free, abandoned-page, statistics, and callback reentrancy paths.
- `scripts/check-behavioral-parity.py` compares the same supported-matrix
  behavioral scenarios against C mimalloc.
- `scripts/check-unix-syscall-parity.py` compares Unix memory primitive syscall
  classes against C mimalloc for representative allocation and purge paths.
