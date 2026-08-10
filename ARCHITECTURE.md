# Architecture

This crate has a small Rust-facing facade over a mostly translated mimalloc
backend. The boundary goal is to make public API commitments explicit while
keeping translated allocator code close enough to upstream to audit.

## Layers

- Tier S, safe Rust facade: `src/safe/` plus the curated crate-root re-exports
  in `src/lib.rs`. This layer owns Rust-native ownership, lifetime, error, and
  snapshot types such as `Allocation`, `Heap`, `Options`, and `StatsSnapshot`.
- Tier U, unsafe Rust conveniences: `src/ffi/callbacks.rs`, re-exported as
  `unsafe_callbacks`. These are public unsafe Rust APIs, not safe facade APIs
  and not C-shaped raw APIs.
- Tier R, raw Rust facade: `src/raw/` and the public C-header mirror in
  `src/ffi/abi.rs` and `src/ffi/headers.rs`. This layer keeps C-shaped names,
  raw pointers, constants, layouts, and compatibility macros visible for tests,
  benchmarks, and low-level integrations.
- Tier C, exported C ABI: exported symbols implemented through `src/ffi/`,
  translated raw modules, explicit interposition shims, and build-mode glue.
  Dynamic exports are validated separately from Rust module visibility.
- Tier I, internal implementation: `src/allocator/`, `src/types/`,
  `src/internal/`, and `src/platform/`. These modules carry allocator state,
  translated helpers, strict wrapper types, platform calls, atomics, and
  provenance-sensitive code. They are not public API.
- Build glue: `build.rs` owns C ABI and interposition build setup, including
  optional C/C++ shims, TLS shims, export scripts, and linker flags.
- Policy gates: `scripts/` and focused tests enforce public boundaries,
  ABI/layout drift, feature behavior, concurrency protocols, and behavioral
  parity.

## Boundary Rules

Tier S may call Tier I only through reviewed backend contacts that enforce
safe ownership, lifetime, nullability, initialization, and error mapping at the
safe boundary. `scripts/check-layer-boundaries.py` keeps those contacts
file-specific so a new safe facade file cannot import allocator internals
without updating the allowlist and this policy.

Tier U must stay under the `unsafe_callbacks` namespace. Callback guards are
process-global and unsafe to construct because synchronization, callback
lifetime, allocator reentrancy, and cross-thread entry cannot be proven by the
safe facade.

Tier R and Tier C keep C compatibility spelling and behavior. Raw macros expand
through public `raw::mi_*` paths and require caller-side unsafe blocks. New raw
modules, exported symbols, layout types, or macro exports must be classified in
`docs/api-inventory.md` and covered by the relevant scripts.

Tier I modules may preserve mechanical C shapes when that improves upstream
parity, but new Rust-facing helper code should prefer typed wrappers, explicit
error states, narrow imports, and small unsafe scopes.

## Maintainer Map

Start with these owners instead of searching the whole tree:

| Concern | Owning code |
|---|---|
| Safe Rust ownership, errors, options, and snapshots | `src/safe/`; curated exports in `src/lib.rs` |
| Raw Rust facade and C-header compatibility | `src/raw/`, `src/ffi/abi.rs`, and `src/ffi/headers.rs` |
| Allocation and reallocation | `src/allocator/alloc.rs`; aligned requests in `src/allocator/aligned.rs` |
| Free routing | `src/allocator/free.rs`; abandoned-page routing in `src/allocator/free/abandoned.rs` |
| Page lifecycle and queues | `src/allocator/page.rs` and its `page/` children |
| Arena lifecycle and OS-backed regions | `src/allocator/arena.rs` and its `arena/` children |
| Process, thread, TLS, and subprocess initialization | `src/allocator/init.rs`; registry links in `init/subproc_registry.rs` |
| Options and diagnostics policy | `src/allocator/options.rs`; Rust facade in `src/safe/options.rs` |
| Statistics mutation, aggregation, and rendering | `src/allocator/stats.rs`; Rust snapshots in `src/safe/stats.rs` |
| Maintained Linux platform primitives | `src/platform/os.rs`, dispatched through `src/platform/mod.rs` and `src/platform/unix.rs` |

The remainder of this section maps every production Rust file above 1,500
lines at the Phase 0 baseline. Category names are those used by the improvement
plan. Extraction notes identify coherent invariant or lifecycle owners; they
are not instructions to split a file merely because it is large.
Public boundary detail lives in the [API inventory](docs/api-inventory.md),
hot-path evidence rules live in the [performance policy](docs/performance.md),
and target status lives in the [platform policy](docs/platforms.md).

### Allocator owners

- `src/allocator/arena.rs` — **raw ABI:** arena lookup and compatibility
  entrypoints; **allocation hot path:** slice claim/commit and arena-page
  allocation/free; **lifecycle:** creation, registration, teardown, and
  abandoned ownership transitions; **policy:** arena suitability and OS
  fallback selection. It owns the mutation protocol for each arena's free, dirty,
  committed, and purge bitmaps and for the subprocess arena table. Page,
  thread-heap, arena-metadata, and abandoned-free paths are the principal
  callers. Existing `arena/` children already own geometry, IDs, OS memory,
  purge, abandoned lists, heap-page visitation, prefixes, and diagnostics.
  Further movement belongs in those owners: abandoned transitions in
  `arena/abandoned.rs`, OS-backed setup in `arena/os_memory.rs`, and complete
  slice-allocation or page-ownership lifecycles in focused child modules.

- `src/allocator/init.rs` — **lifecycle:** static main-process objects, process
  and thread init/done, TLS association, and subprocess lifecycle; **raw ABI:**
  public process/thread/subprocess entrypoints; **platform primitive:** TLS
  keys and CPU feature detection; **policy:** guarded-thread option setup;
  **diagnostics:** initialization failure reporting. Its state includes process once/done flags, main
  subprocess/TLD/heaps, TLS keys, and thread ID allocation. Platform lifecycle
  hooks, thread heaps, pages, arenas, OS accounting, and the safe heap/stats
  backends call it. `init/subproc_registry.rs` already owns intrusive registry
  rewiring. Complete process, thread, TLS, subprocess, and CPU-detection groups
  are the justified child boundaries; raw entrypoints and top-level ordering
  remain visible in the parent.

- `src/allocator/stats.rs` — **allocation hot path:** local and atomic counter
  updates; **lifecycle:** heap, thread, and subprocess aggregation;
  **platform primitive:** clock and process-information collection;
  **diagnostics:** counter snapshots and reports; **formatting:** bounded text
  and JSON rendering; **raw ABI:** C-layout schemas and export shims. Mutable values live on TLDs, heaps, and
  subprocesses; this module owns their update, merge, snapshot, and output
  protocols plus clock and output-buffer state. Allocation, page, arena, OS,
  heap, thread teardown, and the safe stats facade are its callers. Coherent
  owners are counter mutation/aggregation, allocation-free text rendering,
  JSON buffer lifecycle, and process information; raw shims stay in the
  parent.

- `src/allocator/options.rs` — **allocation hot path:** atomic option reads;
  **policy:** descriptors, defaults, parsing, and locked mutation;
  **platform primitive:** bounded environment access; **lifecycle:** option
  initialization and callback registration/restoration; **diagnostics:**
  delayed output, throttling, error dispatch, and assertion abort behavior;
  **formatting:** fixed-buffer messages; **raw ABI:** option and callback
  export shims. Its state includes option atomics/descriptors,
  callback and recursion state, error throttles, and fixed stack buffers.
  Allocator policy, OS/arena code, init, stats rendering, and the safe options
  facade are the main callers. Storage/initialization, environment translation,
  output lifecycle, formatting, and error callback dispatch are complete
  candidate boundaries; keeping fast reads with storage is mandatory.

- `src/allocator/page.rs` — **allocation hot path:** local-list refill/commit,
  cache routing, and generic page allocation; **lifecycle:** queue transitions,
  retirement, and fresh/reclaimed/abandoned acquisition; **policy:** retire and
  search decisions; **diagnostics:** page and queue validation; **raw ABI:**
  C-shaped page entrypoints. It mutates page free lists, counts,
  keys, ownership, and thread-heap queues/direct caches. Allocation, free,
  thread-heap, and arena code are the principal callers. Existing children own
  bins, counts, deferred-free dispatch, flags, lifecycle protocols, queues,
  stats bins, and validation. Move transition and validation work to those
  owners first; refill and acquisition justify new children only as complete
  lifecycles.

- `src/allocator/alloc.rs` — **allocation hot path:** page malloc/zero dispatch,
  usable-size queries, and realloc ownership transfer; **policy:** checked
  layout/product and failure handling; **raw ABI:** malloc/realloc/query export
  families; **diagnostics:** guarded allocation metadata and checks. It holds no module-global mutable state; its invariant
  is validation and transfer of raw user-pointer ownership while updating page
  counts and guarded metadata. Raw/FFI exports, aligned allocation, thread-heap
  allocation, and the safe facade call it. Existing `alloc/` children own
  public malloc, zeroed allocation, C++ new, and string/path families. Realloc,
  guarded allocation, and usable-size/product handling are the remaining
  coherent operation families.

- `src/allocator/aligned.rs` — **allocation hot path:** over-allocation,
  guarded alignment, aligned malloc/calloc, and realloc replacement;
  **policy:** alignment/offset validation and natural-alignment selection;
  **raw ABI:** global and heap aligned export families. It has no global mutable state. Raw exports,
  `src/safe/heap.rs`, `alloc/cxx_new.rs`, and allocation boundary tests are the
  main consumers; `aligned/thread_heap.rs` already owns thread-heap aligned
  allocation. The thread-heap realloc lifecycle belongs with that child; the
  global/heap aligned-realloc ABI family is the only other justified boundary.

### Internal, raw, and platform owners

- `src/internal/bitmap.rs` — **allocation hot path:** atomic chunk mutation,
  bitmap claims, and binned free-slice search; **lifecycle:** sizing,
  initialization, and destructive traversal; **diagnostics:** bin lookup and
  validation helpers; **platform primitive:** optional AVX operations. It also
  defines the bitmap layouts. Arena,
  page-map, and thread-local allocation code are its callers. Existing
  `bitmap/bfield.rs`, `bitmap/chunk_bits.rs`, and `bitmap/chunkmap.rs` own scalar
  fields, chunk operations, and summary maps. Remaining chunk operations belong
  in `chunk_bits.rs`; traversal and the complete binned-bitmap protocol are
  coherent boundaries. Layout constants stay with the top-level API unless a
  real shared owner emerges.

- `src/types/ptr.rs` — **allocation hot path:** page/block/queue metadata and
  user-pointer access; **lifecycle:** heap/TLD, arena, and subprocess metadata;
  **platform primitive:** transparent non-null wrappers, checked address
  helpers, and user/OS pointer conversions. Referent state lives elsewhere; this module
  owns non-null, provenance, field-access, and encoded-link proof boundaries.
  It is called throughout allocator and internal code. Move behavior to the
  existing invariant owners `types/page.rs`, `page_queues.rs`, `heap.rs`,
  `arena.rs`, and `freelist_wrappers.rs`; a subprocess child is justified only
  for its complete registry/counter accessor set. Base wrappers and generic
  address helpers remain here. The synthetic `__lint_touch` chain is deletion
  work for task 1.5, not an extraction candidate.

- `src/raw/macros.rs` — **raw ABI:** macOS interposition, Windows CRT, C++
  new/delete, and Unix/WASI/Linux libc symbols; **allocation hot path:** fast
  override assembly and allocator forwarding; **lifecycle:** Unix bootstrap
  allocation tracking; **platform primitive:** linker sections and per-target
  adapters. Its important state is bootstrap
  allocation tracking and linker-consumed symbol/interposition data; most
  callers are the loader, CRT, or C++ runtime rather than Rust call sites.
  Existing `raw/macros/header_forwarding.rs` and `linux_libc.rs` own forwarding
  and Linux adapters. Bootstrap, fast assembly, and each platform/ABI symbol
  family are coherent move units and must retain their cfg/export attributes
  intact.

- `src/platform/os.rs` — **platform primitive:** guard, commit/decommit, purge,
  protect, allocation/free, and NUMA calls; **lifecycle:** memory configuration
  and huge-page claim/free; **policy:** aligned hints, purge choices, and large
  page selection; **raw ABI:** public OS wrappers. Process configuration, aligned-hint/huge-page cursors, and NUMA
  cache are its global state. Arena, page-map, page commit, init, heap, and stats
-  code and `src/safe/stats.rs` call it; primitive syscalls dispatch through
  `platform/mod.rs` and `platform/unix.rs`. Configuration, virtual-memory lifecycle, huge pages,
  NUMA, and raw wrappers are coherent child boundaries.

- `src/platform/windows.rs` — **platform primitive:** Win32/NT declarations,
  dynamic API slots, virtual memory, and NUMA; **lifecycle:** process/thread/TLS
  setup and teardown; **diagnostics:** ETW provider state and event writing;
  **raw ABI:** imported and exported Windows symbols. This is source-present
  but not a maintained release target; see the
  [platform policy](docs/platforms.md). Its state includes OS version and
  allocation granularity, dynamic function slots, clock/large-page/process
  caches, the FLS key, stderr handles, and ETW enable/provider state.
  `platform/mod.rs`, `platform/os.rs`, and init are its
  internal consumers on Windows. FFI declarations, lifecycle, virtual memory,
  system helpers, NUMA, and ETW are complete boundaries if target-validated
  work later decomposes the file; they must not distort the maintained Linux
  path.
