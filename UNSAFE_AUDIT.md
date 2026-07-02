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
- `src/arena_meta.rs`: arena metadata page stack operations publish and reclaim
  metadata pages through the audited atomic stack discipline.
- `src/atomic.rs`: raw atomic pointer casts are centralized here and mapped to
  C mimalloc atomic operations. Ordering equivalence is covered by
  `ATOMIC_ORDERING_AUDIT.md`.
- `src/bench_diagnostics.rs`: unsafe diagnostic hooks are test/benchmark support
  and do not mutate allocator metadata outside the measured path.
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
- `SAFETY[AUDIT:global-alloc]`: `src/lib.rs` implements `GlobalAlloc` for the
  zero-sized `MiMalloc` facade. Rust's allocator contract supplies the layout,
  pointer, and ownership preconditions, and each operation forwards directly to
  the matching allocator entry point.
- `SAFETY[AUDIT:owned-allocation]`: `src/safe/allocation.rs` owns non-zero
  allocations through move-only wrappers. Slice views and copies are bounded by
  the stored layout, and `Drop` releases live blocks exactly once.
- `SAFETY[AUDIT:panic-boundary]`: `src/lib.rs` no-std panic handling and abort
  declarations terminate instead of unwinding across allocator or C ABI
  boundaries.
- `SAFETY[AUDIT:raw-auto-trait]`: `src/types/constants.rs` implements raw
  auto-traits only for immutable sentinel metadata values whose mutation remains
  protected by allocator ownership, atomics, or locks.
- `SAFETY[AUDIT:raw-macro-boundary]`: `src/ffi/abi.rs` and
  `src/ffi/headers.rs` expose raw C-header macro compatibility without adding
  Rust ownership guarantees; callers inherit the documented raw ABI contract.
- `SAFETY[AUDIT:arena-debug-render]`: `src/allocator/arena/debug.rs` renders
  arena diagnostics by reading arena/page metadata discovered through the
  current sub-process registry, page map lookup, and typed memid views. The
  helpers are diagnostic-only, keep output in stack-local byte buffers, and do
  not allocate, publish, or mutate allocator ownership state.
- `SAFETY[AUDIT:deferred-free-callback-state]`:
  `src/allocator/page/deferred.rs` owns the C-compatible deferred-free callback
  globals, publishes callback and argument with release stores, reads them with
  acquire loads, and invokes callbacks only after bumping the local thread heap
  heartbeat and setting the per-thread-heap reentry guard.
- `SAFETY[AUDIT:safe-heap]`: `src/safe/heap.rs` ties heap allocations to the
  owning heap lifetime, prevents main-heap deletion through owned heap handles,
  and keeps raw heap/block calls behind wrapper ownership checks.
- `SAFETY[AUDIT:safe-process-info]`: `src/safe/stats.rs` passes valid local
  out-pointers to process-info collection and copies returned counters into
  typed Rust values before exposing them.
- `SAFETY[AUDIT:safe-stats]`: `src/safe/stats.rs` initializes stats buffers
  before use, passes valid writable output slices to JSON formatting, and
  exposes only typed totals/counts to callers.
- `SAFETY[AUDIT:typed-freelist]`: `src/types/freelist.rs` reconstructs
  non-null block pointers from packed thread-free words after masking the
  ownership bit. Miri provenance helpers retain the original pointer identity,
  and `BlockPtr` adds only the non-null wrapper invariant.
- `SAFETY[AUDIT:typed-page-test-fixtures]`: `src/types/page/tests.rs` builds
  stack-local page metadata and synthetic page-map range witnesses to exercise
  typed page wrapper accessors. The fixtures are not registered with allocator
  globals and are used only for local invariant tests.
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
  allowlist at 64 entries; removing entries requires lowering that cap in the
  same change. Migrating a file means adding stable operation IDs near unsafe
  operations, documenting each ID in this section, and removing the file from
  the allowlist.

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
