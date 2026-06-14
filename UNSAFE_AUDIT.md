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
- `src/arena_meta.rs`: arena metadata page stack operations publish and reclaim
  metadata pages through the audited atomic stack discipline.
- `src/atomic.rs`: raw atomic pointer casts are centralized here and mapped to
  C mimalloc atomic operations. Ordering equivalence is covered by
  `ATOMIC_ORDERING_AUDIT.md`.
- `src/bitmap.rs`: bitmap word pointers are derived from caller-provided bitmap
  storage and bounded by bit counts; set/clear/claim transitions use atomic word
  operations equivalent to C.
- `src/free.rs`: free paths recover page metadata from owned block pointers,
  validate page ownership where C does, and route remote frees through delayed
  free queues instead of mutating foreign-thread page state directly.
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

Validation:

- `scripts/check-unsafe-audit.py` verifies every `src/` file containing `unsafe`
  is listed here and that the audit references the related atomic-ordering and
  tracked-mode follow-up audits.
- `cargo test` exercises the public ABI, heap/theap/subprocess lifecycle,
  cross-thread free, abandoned-page, statistics, and callback reentrancy paths.
- `scripts/check-behavioral-parity.py` compares the same supported-matrix
  behavioral scenarios against C mimalloc.
- `scripts/check-unix-syscall-parity.py` compares Unix memory primitive syscall
  classes against C mimalloc for representative allocation and purge paths.
