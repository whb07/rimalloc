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
- `SAFETY[AUDIT:ffi-libc-helpers]`: `src/ffi/libc.rs` owns libc-like helper
  routines for bounded C-string scans, destination writes, fixed-buffer
  formatting, Unix `vsnprintf` forwarding, numeric output reversal, and
  environment lookup forwarding. Callers provide C-compatible readable strings,
  writable output buffers, and valid varargs objects where required.
- `SAFETY[AUDIT:global-alloc]`: `src/lib.rs` implements `GlobalAlloc` for the
  zero-sized `MiMalloc` facade. Rust's allocator contract supplies the layout,
  pointer, and ownership preconditions, and each operation forwards directly to
  the matching allocator entry point.
- `SAFETY[AUDIT:heap-lifecycle-boundary]`: `src/allocator/heap.rs` owns raw
  heap lifecycle and ownership-inspection shims. Unsafe operations are limited
  to heap creation/registration, NUMA affinity updates, stats merging, owned
  heap delete/destroy teardown, allocator page ownership lookups,
  under-utilized page inspection, and lazy per-heap thread-heap initialization
  while preserving typed heap witnesses and lock-protected list updates.
- `SAFETY[AUDIT:thread-heap-lifecycle]`: `src/allocator/thread_heap.rs` owns
  thread-heap visitors, collection, default switching, initialization,
  creation, free/delete, refcount, random, stats-merge, and arena-memory cleanup
  boundaries. Unsafe operations stay at C ABI shims, callback invocations,
  page/block traversal, fast-divisor out-pointers, and caller-proven
  heap/thread-heap/TLD metadata transitions routed through typed wrappers.
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
- `SAFETY[AUDIT:init-thread-lifecycle-boundary]`: `src/allocator/init.rs` owns
  public thread initialization and teardown. Unsafe operations are limited to
  current-thread witnesses, default thread-heap initialization, local
  thread-heap cleanup, stat updates, and C-equivalent fallback paths during
  thread done/init.
- `SAFETY[AUDIT:init-thread-heap-tls-boundary]`: `src/allocator/init.rs` owns
  default, cached, empty, and process-main thread-heap TLS association. Unsafe
  operations are limited to typed thread-heap wrappers, TLS reads and writes,
  main-thread heap publication, and no-allocation default/cached switching.
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
- `SAFETY[AUDIT:allocation-abi-entry-boundary]`: `src/allocator/alloc.rs` owns
  public C allocation, string, realpath, realloc-family, and C++ new/delete
  entry shims. Unsafe operations are limited to caller-proven ABI pointers,
  errno slots, exported symbol wrappers, old-allocation validation handoff, and
  new-handler callbacks while preserving C null/failure semantics and raw
  ownership transfer.
- `SAFETY[AUDIT:allocation-heap-dispatch-boundary]`:
  `src/allocator/alloc.rs` owns thread-heap and heap allocation dispatch for
  small, zeroed, calloc, and typed allocation requests. Unsafe operations are
  limited to typed heap/thread-heap witnesses, usable-size out-pointers, page
  free-list inspection, and tracking handoff while preserving no-allocation
  hot paths.
- `SAFETY[AUDIT:allocation-layout-request-boundary]`:
  `src/allocator/alloc.rs` owns allocation request construction, product
  overflow handling, layout padding, realloc-size validation, and old-block
  usable-size recovery. Unsafe operations are limited to caller-proven
  allocation pointers, typed layout witnesses, and bounded metadata queries
  while preserving overflow and realloc copy bounds.
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
- `SAFETY[AUDIT:page-queue-transition-boundary]`:
  `src/allocator/page.rs` owns page queue movement, full/unfull transitions,
  retire/free handoff, abandon/reclaim, queue validation, and queue append
  operations. Unsafe operations are limited to typed page-state witnesses,
  caller-proven heap/queue/page pointers, and arena page free/abandon handoff
  while preserving queue links, heap ownership, and page counters.
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
- `SAFETY[AUDIT:internal-bitmap-boundary]`: `src/internal/bitmap.rs` owns
  bitmap and binned-bitmap chunk maps, bit scanning, set/clear mutation,
  first-fit claim loops, arena traversal callbacks, and AVX2 bitmap helper
  shims. Unsafe operations are limited to caller-proven bitmap storage,
  bounded chunk and field projection, raw callback invocation, typed
  arena/slice wrapper handoff, and local test fixtures while preserving bit
  ordering, field width, empty/full handling, and allocation-free search paths.
- `SAFETY[AUDIT:internal-translation-boundary]`: `src/internal/mod.rs` owns
  translated internal header helpers, page and block pointer lookup, page
  metadata accessors, thread-free ownership words, packed pointer
  encode/decode, tracking gates, CPU copy/fill shims, and C-compatible helper
  declarations. Unsafe operations are limited to caller-proven allocator
  metadata pointers, page-map projection, typed atomic field wrappers, encoded
  free-list words, provenance-aware pointer/integer conversions, inline
  assembly copy/fill thresholds, and raw helper shims while preserving the
  local ABI shape and allocation-free hot paths.
- `SAFETY[AUDIT:free-path-boundary]`: `src/allocator/free.rs` owns free and
  usable-size routing. Unsafe operations are limited to free fast-path assembly
  aliases, validated page/block/user-pointer lookup, padding and guarded-block
  metadata, local and remote free-list mutation, thread-free atomic
  publication, abandoned-page collect/reclaim/reabandon decisions, usable-size
  shims, and raw C ABI free exports while preserving allocator ownership and
  no-allocation hot-path constraints.
- `SAFETY[AUDIT:aligned-padding-boundary]`:
  `src/allocator/aligned.rs` owns padding shrink/grow behavior, over-allocation
  trimming, guarded aligned allocation, interior-pointer tagging, and tracking
  metadata for adjusted aligned blocks. Unsafe operations are limited to
  padding-shrink shims, page/block metadata, padding metadata writes, and
  tracking hooks while preserving usable-size and offset invariants.
- `SAFETY[AUDIT:aligned-heap-allocation-boundary]`:
  `src/allocator/aligned.rs` owns heap and thread-heap aligned malloc, zalloc,
  calloc, and zeroed aligned entry paths after safe request normalization.
  Unsafe operations are limited to thread-heap local witnesses, aligned page
  free-list checks, usable-size out-pointers, heap borrowing, guarded handoff,
  and raw aligned allocation exports while preserving alignment and hot-path
  dispatch constraints.
- `SAFETY[AUDIT:aligned-realloc-boundary]`:
  `src/allocator/aligned.rs` owns aligned realloc, recalloc, offset-aware
  realloc request routing, old-allocation validation, copy preservation, and
  raw aligned realloc exports. Unsafe operations are limited to caller-proven
  old allocation pointers, heap borrowing, aligned allocation/free handoff, and
  bounded copy metadata while preserving old contents and realloc failure
  semantics.
- `SAFETY[AUDIT:options-callback-boundary]`: `src/allocator/options.rs` owns
  option descriptors, output/error callback state, recursion guards,
  environment parsing, diagnostic output, option raw exports, and assert abort
  shims. Unsafe operations are limited to extern C declarations, locked global
  option/callback state, C string reads, bounded stack output buffers,
  primitive stderr forwarding, callback invocation, environment lookup, and
  raw C ABI option exports while preserving no allocator allocation in
  primitive output paths.
- `SAFETY[AUDIT:stats-counter-boundary]`: `src/allocator/stats.rs` owns stats
  initialization and counter/count mutation helpers. Unsafe operations are
  limited to POD zero initialization, checked bin indexes, and atomic counter
  field access while preserving counter width and stat-level gating.
- `SAFETY[AUDIT:stats-merge-boundary]`: `src/allocator/stats.rs` owns stats
  merge, copy, reset, heap aggregation, subproc aggregation, and lifecycle
  merge exports. Unsafe operations are limited to raw stats pointer validation,
  non-null merge bodies, heap/subproc witness construction, visitor callbacks,
  and memzero reset handoff while preserving aggregate semantics.
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
- `SAFETY[AUDIT:raw-macro-libc-forwarding-boundary]`: `src/raw/macros.rs` owns
  Unix, Linux libc, WASI libc, POSIX, and libc-adjacent allocation forwarding
  shims. Unsafe operations are limited to C-shaped malloc/free/realloc/calloc,
  strdup/strndup, valloc/pvalloc, memalign/posix_memalign, reallocarray, and
  symbol alias forwarding while preserving raw libc ownership contracts.
- `SAFETY[AUDIT:raw-macro-allocation-query-boundary]`: `src/raw/macros.rs`
  owns malloc size, usable-size, and good-size compatibility queries. Unsafe
  operations are limited to checked heap-region probes and raw forwarding to
  allocator query exports, including bootstrap allocation size queries.
- `SAFETY[AUDIT:raw-macro-cxx-boundary]`: `src/raw/macros.rs` owns C++ operator
  new/delete compatibility symbols and nothrow/aligned/sized variants. Unsafe
  operations are limited to exported C++ ABI symbol shims and direct forwarding
  to the corresponding allocator allocation or free entry points.
- `SAFETY[AUDIT:raw-macro-header-forwarding-boundary]`: `src/raw/macros.rs`
  owns header-shaped forwarding helpers used by raw macro compatibility layers.
  Unsafe operations are limited to calling caller-supplied raw C function
  pointers or forwarding to raw allocator exports without adding Rust ownership
  guarantees.
- `SAFETY[AUDIT:raw-posix-compat]`: `src/raw/posix.rs` owns POSIX and
  libc-adjacent raw compatibility helpers. Unsafe operations are limited to
  errno slot publication, environment lookup, raw allocation forwarding,
  POSIX-compatible out-pointer and pointer-slot writes, wide-string scanning and
  copying, environment duplication, and aligned/product allocation shims.
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
- `SAFETY[AUDIT:os-memory-allocation-boundary]`: `src/platform/os.rs` owns OS
  allocation, zero-allocation, aligned allocation, trimming, and public OS
  allocation shims. Unsafe operations are limited to primitive allocation
  handoff, typed reserve/commit witnesses, memid publication, out-pointer
  writes, and alignment trimming while preserving zero-state and ownership.
- `SAFETY[AUDIT:os-memory-huge-page-boundary]`: `src/platform/os.rs` owns huge
  OS page detection, reservation, claiming, and aligned huge allocation.
  Unsafe operations are limited to primitive huge-page calls, arena claim
  callback handoff, per-segment memid state, and huge-page free rollback while
  preserving huge-page size and NUMA placement contracts.
- `SAFETY[AUDIT:os-memory-numa-boundary]`: `src/platform/os.rs` owns NUMA node
  count and current-node helper shims. Unsafe operations are limited to
  primitive NUMA calls and retained C ABI wrappers while preserving nullable
  and fallback behavior.
- `SAFETY[AUDIT:macos-zone-override]`: `src/platform/macos.rs` owns the
  source-present macOS malloc-zone override boundary. Unsafe operations are
  limited to Darwin malloc-zone callback signatures and imports, mutable
  process-global zone metadata, nullable out-pointer writes, default-zone
  ordering during process initialization, non-mimalloc zone forwarding, mimalloc
  ownership queries, batch pointer-array reads/writes, fork-hook exports, and
  no-op compatibility shims.
- `SAFETY[AUDIT:platform-tls-primitives]`: `src/platform/mod.rs` owns
  cross-platform primitive TLS and thread-heap dispatch. Unsafe operations are
  limited to platform TLS declarations, Linux x86_64 initial-exec assembly,
  Apple fixed-slot reads, dynamic TLS key lookups, raw default/cached
  thread-heap get/set helpers, empty-thread-heap fallbacks, page-associated
  thread-heap lookup, and allocator init/done compatibility shims.
- `SAFETY[AUDIT:internal-atomic-field-boundary]`: `src/internal/atomic.rs`
  owns scalar C-compatible atomic field views and load/store/RMW/CAS wrappers.
  Unsafe operations are limited to raw `AtomicUsize`/`AtomicIsize`/`AtomicI64`
  views over caller-proven storage, memory-order mapping helpers, expected-slot
  updates, volatile addend reads, and scalar atomic helper forwarding.
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
- `SAFETY[AUDIT:page-map-init-boundary]`: `src/allocator/page_map.rs` owns
  32-bit and non-32-bit page-map initialization, reserve/commit setup, null
  sentinel publication, and lazy submap growth. Unsafe operations are limited
  to OS reserve/commit calls, typed index/count witnesses, lock-protected
  submap allocation, and acquire/release publication.
- `SAFETY[AUDIT:page-map-teardown-boundary]`: `src/allocator/page_map.rs` owns
  page-map unsafe destroy paths. Unsafe operations are limited to lifecycle
  exclusive global reads, recorded memid witnesses, submap iteration, and OS
  free calls while preserving teardown after all users are gone.
- `SAFETY[AUDIT:page-map-register-boundary]`: `src/allocator/page_map.rs` owns
  page register, unregister, unregister-range, and range set/clear helpers.
  Unsafe operations are limited to raw page validation, typed page range
  witnesses, committed-range checks, sub-index writes, and
  `RegisteredPageRange` construction.
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
- `SAFETY[AUDIT:wasi-primitive-boundary]`: `src/platform/wasi.rs` owns the
  bounded WASI primitive backend. Unsafe operations are limited to C import
  declarations, `sbrk` memory growth, nullable primitive out-pointer conversion,
  bounded C-string stderr and environment handling, no-op primitive range shims,
  unsupported huge-page result publication, and thread/TLS compatibility shims.
- `SAFETY[AUDIT:emscripten-primitive-boundary]`:
  `src/platform/emscripten.rs` owns the bounded Emscripten primitive backend.
  Unsafe operations are limited to Emscripten import declarations, emmalloc
  allocation/free calls, nullable primitive out-pointer conversion, clock and
  console calls, host stderr fallback C-string handling, unsupported environment
  and huge-page shims, pthread key lifecycle, static key mutation, and fallback
  thread-local association.
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
- `SAFETY[AUDIT:arena-heap-page-visitors]`:
  `src/allocator/arena/heap_pages.rs` owns heap page visitation, public C
  callback traversal, and heap page move/destroy helpers. Callers supply live
  heap/page metadata, and the helpers wrap raw pointers in typed heap/page
  witnesses before walking arena bitmaps, claiming page ownership, or moving
  pages between heap arena-page tables.
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
- `SAFETY[AUDIT:threadlocal-key-manager]`: `src/allocator/threadlocal.rs`
  owns dynamic thread-local key bookkeeping, including Linux TLS shim access,
  x86_64 initial-exec assembly, static lock use, slot block growth, global free
  bitmap publication, callback keep-set writes, and versioned key claim/free
  operations.
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
- `SAFETY[AUDIT:typed-atomic-domain]`: `src/types/atomic.rs` owns typed
  atomic field wrappers for allocator metadata. Unsafe operations are limited
  to temporary `AtomicUsize`/`AtomicI64`/`AtomicPtr` views over caller-proven raw
  fields, typed page thread-id and thread-free word RMW/CAS protocols,
  abandoned-count and thread-heap refcount atomics, arena page-table pointer
  atomics, once/guard/lock RAII boundaries, and lint-only non-dereferenced
  identity fixtures.
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
- `SAFETY[AUDIT:typed-heap-test-fixtures]`: `src/types/heap/tests.rs` builds
  stack-local heap, thread-heap, and TLD metadata to exercise heap witness
  constructors and thread-local promotion checks. Real heaps allocated by the
  tests are released exactly once, and stack-local records are used only while
  their fixtures are live.
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
- `SAFETY[AUDIT:typed-pointer-test-fixtures]`: `src/types/ptr/tests.rs`
  builds stack-local page, block, heap, thread-heap, TLD, arena, subproc, and
  page-queue metadata to exercise pointer wrapper field accessors and exposed
  address arithmetic. The fixtures remain stack-local, are not registered with
  allocator globals, and use non-dereferenced extreme/dangling pointers only
  for checked overflow and underflow cases.
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
