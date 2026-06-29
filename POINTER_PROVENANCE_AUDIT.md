# Pointer Provenance and Integer Arithmetic Audit

Scope: Rust sources in `src/` that perform pointer arithmetic, pointer/integer
casts, wrapping arithmetic, checked arithmetic, or alignment calculations. This
audit covers the supported default parity matrix documented in `PARITY.md`.

Miri was checked for this toolchain but is not available:
`cargo miri --version` reports that the `miri` component is unavailable for
`stable-x86_64-unknown-linux-gnu`. The UB-oriented validation available here is
therefore the repository boundary tests, C-vs-Rust behavioral parity probes, and
the guard script below.

Shared invariants:

- Pointer-to-integer casts are used for C-equivalent alignment checks, page-map
  indexing, encoded free-list links, sentinel values, and OS address accounting.
  They are not used to manufacture unrelated object provenance before
  dereference.
- `.add`, `.sub`, and `offset_from` are used only within allocator-owned arrays,
  page blocks, arena ranges, metadata pages, buffers, or OS mappings selected by
  the surrounding allocator checks.
- Count/size multiplication that accepts public input goes through checked
  arithmetic before allocation. Internal multiplications use bounded mimalloc
  metadata fields such as page capacity, slice counts, or fixed bin sizes.
- Alignment helpers preserve the C contract: callers pass power-of-two allocator
  alignments, and public APIs reject or characterize invalid alignments to match
  mimalloc C behavior.
- Wrapping arithmetic is limited to bit operations, hash/random mixing, encoded
  free-list cookies, and sentinel-safe alignment masks where C intentionally uses
  unsigned wraparound.

Per-file review:

- `src/abi.rs`: public ABI declarations preserve raw C pointer shapes for
  external callers; no ownership is created by declaration-only casts.
- `src/alloc.rs`: public count/size allocation paths use overflow helpers before
  allocating; page and block pointer use is delegated to checked heap/page
  helpers.
- `src/alloc_aligned.rs`: adjusted aligned pointers are computed after
  alignment/offset validation and retain the original allocation metadata for
  deallocation.
- `src/alloc_override.rs`: pointer casts are ABI shim conversions and forward to
  public allocator APIs.
- `src/alloc_posix.rs`: POSIX alignment and output-pointer paths validate public
  inputs before pointer writes or allocation.
- `src/arena.rs`: arena start, slice metadata, bitmap storage, and page starts
  are derived from registered arena ranges. Slice and bitmap offsets are bounded
  by arena size, slice counts, and `MI_ARENA_SLICE_SIZE`.
- `src/arena_meta.rs`: metadata page pointer arithmetic stays within guarded
  metadata page mappings and checks block indexes/counts against `mi_memid_t`.
- `src/atomic.rs`: pointer casts centralize C-compatible atomic access to
  allocator-owned atomic fields; ordering is covered by `ATOMIC_ORDERING_AUDIT.md`.
- `src/bench_diagnostics.rs`: diagnostic pointer and integer markers are limited
  to test/benchmark instrumentation and do not feed allocator ownership.
- `src/bitmap.rs`: bit indexes are converted to bitmap word indexes with bounded
  counts; scan and claim loops stay within the caller-provided bitmap storage.
- `src/bits.rs`: bit-scan, rotate, and power-of-two helpers use integer-only
  arithmetic equivalent to C intrinsics.
- `src/core_types/arena.rs`, `src/core_types/atomic_wrappers.rs`,
  `src/core_types/callbacks.rs`, `src/core_types/freelist.rs`,
  `src/core_types/freelist_wrappers.rs`, `src/core_types/guarded.rs`,
  `src/core_types/layout.rs`, `src/core_types/memid.rs`,
  `src/core_types/padding.rs`, `src/core_types/ptr.rs`,
  `src/core_types/scalar.rs`, and `src/core_types/track.rs`: typed wrappers
  encode allocator layout, pointer, scalar, callback, and tracking invariants;
  arithmetic remains a checked facade over the same C-compatible metadata.
- Nested core-type tests with pointer and arithmetic fixtures cover these wrapper
  invariants without creating public API:
  `src/core_types/arena/tests.rs`,
  `src/core_types/atomic_wrappers/tests.rs`,
  `src/core_types/freelist/tests.rs`,
  `src/core_types/memid/tests.rs`,
  `src/core_types/padding/tests.rs`,
  `src/core_types/ptr/tests.rs`, and
  `src/core_types/scalar/tests.rs`.
- `src/free.rs`: free paths compute block starts, padding locations, and guarded
  page addresses only after page lookup/ownership checks.
- `src/headers.rs`: macro casts preserve C header API types and forward to the
  allocator APIs that perform validation.
- `src/heap.rs`: sentinel pointer values are never dereferenced; percentage math
  uses bounded page counters.
- `src/init.rs`: TLS sentinels and CPUID register-buffer writes use fixed-size
  arrays or documented sentinel values.
- `src/internal.rs`: core page-map, page/block, alignment, memid, and encoded
  free-list helpers mirror C layout formulas. Public overflow checks are
  centralized here.
- `src/libc.rs`: pointer casts are libc ABI declarations and thin platform call
  adapters.
- `src/mimalloc.rs`: pointer casts are public ABI declarations for callers and
  tests.
- `src/options.rs`: enum indexes are bounded by `_mi_option_last`; size suffix
  parsing uses checked multiplication before returning byte counts.
- `src/os.rs`: OS range arithmetic uses aligned size/page helpers and memid range
  fields before commit, decommit, reset, protect, or free operations.
- `src/page.rs`: page free-list construction and block iteration use capacity,
  reserved count, block size, and page area bounds from initialized page
  metadata.
- `src/page_map.rs`: submap indexes are derived from arena slice indexes and
  checked against page-map/submap dimensions before publication and lookup.
- `src/prim.rs`: TLS pointer casts use fixed slot indexes and expansion-slot
  bounds.
- `src/prim/osx.rs`: malloc-zone ABI pointer casts match platform struct
  layouts and forward allocation work to public APIs.
- `src/prim/unix.rs`: environment, random, mmap, and TLS pointer handling uses
  platform return checks and explicit size values from the OS.
- `src/prim/wasi.rs`: WASI primitive pointer handling validates unsupported or
  failed operations by returning C-compatible failures.
- `src/prim/windows.rs`: Windows pointer/integer conversions are used for
  `VirtualAlloc` ranges, environment buffers, TLS/FLS, and system information
  fields after platform result checks.
- `src/random.rs`: wrapping arithmetic is intentional ChaCha/hash mixing; context
  pointer values are used only as entropy seeds, not dereferenced after integer
  conversion.
- `src/safe.rs`: safe facade pointer casts are limited to forwarding caller-owned
  handles into the raw ABI after preserving the public API contract.
- `src/safe/allocations.rs`, `src/safe/heaps.rs`, and `src/safe/stats.rs`: safe
  wrappers expose raw pointer escape hatches, heap allocation handles, and stats
  buffers only after preserving wrapper ownership and bounds.
- `src/stats.rs`: output-buffer pointer increments are bounded by buffer size;
  stat-to-process-info conversions clamp negative counters before casting.
- `src/theap.rs`: page/block visitors walk initialized page areas with capacity
  and block-size bounds and check block addresses against page ranges.
- `src/threadlocal.rs`: TLS storage layout uses checked size arithmetic before
  allocation/reallocation, and slot indexing is bounded by the current slot count.
- `src/types.rs`: max-allocation constants and C layout declarations preserve
  mimalloc's size limits and ABI shapes.

Operation IDs:

- `PROVENANCE[AUDIT:packed-thread-free-word]`: `src/types/freelist.rs`
  converts between a block pointer and the packed `xthread_free` word. Only the
  low ownership bit is mixed with the exposed block address, null remains the
  zero word, and Miri builds resolve remembered addresses before rebuilding a raw
  pointer.

Migration:

- Operation-level enforcement is incremental. `scripts/check-pointer-provenance-audit.py`
  verifies every source `PROVENANCE[AUDIT:<id>]` is listed exactly once here
  while preserving the existing file-level pointer/arithmetic inventory.
- New pointer/integer round trips, raw alignment arithmetic, or packed-pointer
  conversions should add a stable operation ID and invariant entry before the
  surrounding file is treated as migrated from file-level to operation-level
  coverage.

Validation:

- `scripts/check-pointer-provenance-audit.py` verifies every `src/` file with
  pointer/arithmetic patterns is listed here and checks operation IDs.
- `cargo test` covers public boundary cases including overflow, alignment,
  usable-size, heap/theap ownership, and lifecycle tests.
- `scripts/check-behavioral-parity.py` compares C and Rust behavior for
  allocation-size boundaries, alignment matrices, good-size/bin boundaries,
  managed memory ranges, arena containment, heap ownership, and stats counters.
- `scripts/check-unix-syscall-parity.py` checks representative Unix OS memory
  primitive range behavior against C mimalloc.
