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
- `SAFETY[AUDIT:init-lifecycle-boundary]`: `src/allocator/init.rs` owns process
  and thread initialization/teardown, preloading state, subproc identity
  conversion, main heap/TLS association, constructor/destructor hooks, random
  and CPU feature setup, and raw lifecycle exports. Unsafe operations are
  limited to C ABI shims, process-global metadata publication, TLS slot/key
  setup, typed subproc/thread-heap witness construction, statically linked
  assembly symbols, option/stat initialization ordering, and teardown paths
  while preserving idempotent initialization and no-unwind lifecycle behavior.
- `SAFETY[AUDIT:allocation-entry-boundary]`: `src/allocator/alloc.rs` owns
  malloc/calloc/realloc request construction, thread-heap and heap dispatch,
  small-page allocation, zeroing, padding/provenance recovery, product
  overflow checks, raw allocation exports, and new/nothrow shims. Unsafe
  operations are limited to caller-proven raw allocation pointers, typed layout
  witnesses, page/block metadata reached through allocator ownership, C ABI
  entry points, and local test fixtures while preserving failure/null
  semantics, realloc copy bounds, and no-allocation hot-path constraints.
- `SAFETY[AUDIT:internal-bitmap-boundary]`: `src/internal/bitmap.rs` owns
  bitmap and binned-bitmap chunk maps, bit scanning, set/clear mutation,
  first-fit claim loops, arena traversal callbacks, and AVX2 bitmap helper
  shims. Unsafe operations are limited to caller-proven bitmap storage,
  bounded chunk and field projection, raw callback invocation, typed
  arena/slice wrapper handoff, and local test fixtures while preserving bit
  ordering, field width, empty/full handling, and allocation-free search paths.
- `SAFETY[AUDIT:free-path-boundary]`: `src/allocator/free.rs` owns free and
  usable-size routing. Unsafe operations are limited to free fast-path assembly
  aliases, validated page/block/user-pointer lookup, padding and guarded-block
  metadata, local and remote free-list mutation, thread-free atomic
  publication, abandoned-page collect/reclaim/reabandon decisions, usable-size
  shims, and raw C ABI free exports while preserving allocator ownership and
  no-allocation hot-path constraints.
- `SAFETY[AUDIT:aligned-allocation-boundary]`:
  `src/allocator/aligned.rs` owns alignment-sensitive allocation, aligned
  realloc, offset-aware layout adjustment, usable-size preservation,
  over-allocation trimming, and original-allocation pointer recovery. Unsafe
  operations are limited to padding-shrink assembly shims, thread-heap local
  witnesses, guarded and aligned allocation calls, page/free-list inspection,
  padding metadata updates, tracking hooks, realloc old-allocation validation,
  heap borrowing, and raw C ABI aligned exports while preserving alignment,
  offset, zeroing, usable-size, and hot-path dispatch constraints.
- `SAFETY[AUDIT:options-callback-boundary]`: `src/allocator/options.rs` owns
  option descriptors, output/error callback state, recursion guards,
  environment parsing, diagnostic output, option raw exports, and assert abort
  shims. Unsafe operations are limited to extern C declarations, locked global
  option/callback state, C string reads, bounded stack output buffers,
  primitive stderr forwarding, callback invocation, environment lookup, and
  raw C ABI option exports while preserving no allocator allocation in
  primitive output paths.
- `SAFETY[AUDIT:stats-reporting-boundary]`: `src/allocator/stats.rs` owns
  counter updates, stats merge/reset/copy, formatted stats output, process
  info, JSON output buffers, heap/subproc aggregation visitors, and raw C stats
  exports. Unsafe operations are limited to POD zero initialization, atomic
  counter field access, raw stats merge/copy helpers, checked bin indexes,
  callback output buffers, heap/subproc witness construction, process-info
  out-pointers, JSON buffer allocation/free, legacy callback pointer
  transmutation, and C ABI shims while preserving counter semantics, output
  buffer bounds, and allocator recursion constraints.
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
- `SAFETY[AUDIT:raw-macro-allocation-boundary]`: `src/raw/macros.rs` owns the
  raw allocation override facade for C, C++, libc, WASI, Linux libc, macOS
  interposition, fast assembly aliases, bootstrap allocation tracking, and
  header-shaped forwarding helpers. Unsafe operations are limited to exported
  ABI attributes, foreign allocation declarations, platform interpose records,
  assembly override entry points, bootstrap `mmap`/`munmap` tracking, raw
  forwarding to allocator exports, C++ operator symbol shims, and test fixtures
  that exercise bootstrap allocation ownership.
- `SAFETY[AUDIT:raw-posix-compat]`: `src/raw/posix.rs` owns POSIX and
  libc-adjacent raw compatibility helpers. Unsafe operations are limited to
  errno slot publication, environment lookup, raw allocation forwarding,
  POSIX-compatible out-pointer and pointer-slot writes, wide-string scanning and
  copying, environment duplication, and aligned/product allocation shims.
- `SAFETY[AUDIT:unix-primitive-backend]`: `src/platform/unix.rs` owns the Unix
  primitive backend. Unsafe operations are limited to libc/syscall imports,
  proc/sys probes, `sysconf`, `mmap`/`munmap`/`madvise`/`mprotect`, huge-page
  and NUMA syscalls, process-info writes, stderr writes, environment lookup,
  entropy fallbacks, pthread destructor keys, and scheduler yield while keeping
  primitive helpers allocation-free and C ABI compatible.
- `SAFETY[AUDIT:os-memory-boundary]`: `src/platform/os.rs` owns the typed OS
  memory boundary above the primitive backends. Unsafe operations are limited
  to memory-config reads and writes, sub-process stats projection, page-size
  and alignment queries, aligned hint generation, guard-page callbacks,
  commit/decommit/reset/reuse/purge/protect/free paths, aligned allocation and
  trimming, zero-state publication, huge-page claim/free, NUMA helpers, and raw
  OS shims while preserving memid commit/zero/pinned semantics and avoiding
  allocator recursion.
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
  thread-heap lookup, process attach/detach callbacks, and allocator
  init/done compatibility shims.
- `SAFETY[AUDIT:internal-atomic-facade]`: `src/internal/atomic.rs` owns the
  C-compatible atomic, once, and lock facade. Unsafe operations are limited to
  Linux pthread mutex imports and calls, raw `Atomic*` views over
  caller-proven storage, memory-order mapping helpers, load/store/RMW/CAS
  wrappers, pointer-sized atomic helpers, volatile addend reads, guard/once
  entry shims, fallback spin locks, and release Linux lock init/done behavior.
- `SAFETY[AUDIT:page-map-boundary]`: `src/allocator/page_map.rs` owns allocator
  page registration, lookup, and teardown. Unsafe operations are limited to
  process-global page-map metadata, OS reserve/commit/free calls, 32-bit commit
  bitmap access, typed page span/index conversion, lock-protected submap
  allocation and acquire/release publication, page range set/clear/register/
  unregister operations, `RegisteredPageRange` witness construction, safe
  arbitrary-pointer lookup, and heap-region query forwarding.
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
- `SAFETY[AUDIT:typed-pointer-accessors]`: `src/types/ptr.rs` owns transparent
  `NonNull` pointer wrappers and raw metadata accessors. Unsafe operations are
  limited to caller-proven non-null construction, raw field reads/writes,
  atomic field projections, encoded free-list link access, page/block address
  arithmetic, heap/thread-heap/TLD/arena/subproc/page-queue accessors, scalar
  narrowing after local assertions, lock pointer projections, and lint-touch
  function-pointer references without allocation or dynamic dispatch.
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
