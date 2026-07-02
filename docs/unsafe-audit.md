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
  slice boundaries, OS out-parameters, and operation-level OS allocation plus
  page-map bootstrap audit tags tracked in `UNSAFE_AUDIT.md`.
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

## Typed Fixture Audit Tags

Typed proof fixtures are split by the invariant under test instead of by source
file:

- `SAFETY[AUDIT:typed-page-pointer-test-fixtures]`,
  `SAFETY[AUDIT:typed-block-pointer-test-fixtures]`,
  `SAFETY[AUDIT:typed-heap-pointer-test-fixtures]`,
  `SAFETY[AUDIT:typed-thread-heap-pointer-test-fixtures]`,
  `SAFETY[AUDIT:typed-tld-pointer-test-fixtures]`,
  `SAFETY[AUDIT:typed-pointer-address-test-fixtures]`,
  `SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]`,
  `SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]`, and
  `SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]` cover
  `src/types/ptr/tests.rs` stack-local metadata, encoded-link, heap,
  thread-heap, TLD, raw field-address, accessor, queue, and
  exposed-address/provenance fixtures independently.
- `SAFETY[AUDIT:typed-main-heap-test-fixtures]`,
  `SAFETY[AUDIT:typed-owned-heap-test-fixtures]`,
  `SAFETY[AUDIT:typed-borrowed-heap-test-fixtures]`,
  `SAFETY[AUDIT:typed-thread-heap-state-test-fixtures]`, and
  `SAFETY[AUDIT:typed-thread-local-heap-test-fixtures]` cover
  `src/types/heap/tests.rs` main, owned, borrowed, thread-heap state, and
  thread-local promotion witnesses independently.
- `SAFETY[AUDIT:typed-callback-test-fixtures]` and
  `SAFETY[AUDIT:typed-callback-invocation-test-fixtures]` cover callback
  wrapper layout/storage fixtures separately from local C ABI invocation
  fixtures in `src/types/callbacks/tests.rs`.
- `SAFETY[AUDIT:typed-page-queue-test-fixtures]` and
  `SAFETY[AUDIT:typed-page-queue-membership-test-fixtures]` cover page-queue
  classification fixtures separately from linked/unlinked membership fixtures
  in `src/types/page_queues/tests.rs`.

## Boundary Audit Tags

- `SAFETY[AUDIT:global-alloc]`: the `MiMalloc` `GlobalAlloc` facade. Callers
  provide valid non-zero `Layout` values and live allocation pointers according
  to Rust's allocator contract. The callee dispatches to normal or aligned
  mimalloc paths, returns null on failure, preserves reallocation contents on
  success, and does not unwind intentionally.
- `SAFETY[AUDIT:owned-allocation-traits-boundary]`,
  `SAFETY[AUDIT:owned-allocation-slice-boundary]`,
  `SAFETY[AUDIT:owned-allocation-resize-boundary]`,
  `SAFETY[AUDIT:owned-initialized-allocation-boundary]`, and
  `SAFETY[AUDIT:owned-allocation-test-fixtures]`: the safe allocation wrappers
  own allocations through move-only handles, split raw slice views, resize/drop
  ownership transfer, initialized-byte views, and local fixtures into separate
  audit surfaces.
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
- `SAFETY[AUDIT:raw-macro-bootstrap-boundary]`: Unix bootstrap allocation
  tracking in `src/raw/macros.rs`, including mmap-backed bootstrap blocks,
  preloading handoffs, and free/realloc ownership transfer.
- `SAFETY[AUDIT:raw-macro-fast-override-boundary]`: fast override assembly
  aliases in `src/raw/macros.rs` for C malloc/free and C++ new/delete ABI
  entrypoints.
- `SAFETY[AUDIT:raw-macro-macos-interpose-boundary]`: macOS interpose tables
  and foreign allocation target declarations in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-windows-crt-boundary]`: Windows CRT allocation
  exports in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-windows-aligned-boundary]`: Windows aligned CRT
  allocation exports in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-unix-core-boundary]`: Unix malloc/calloc/realloc/free
  export shims in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-unix-string-boundary]`: Unix strdup/strndup forwarding
  shims in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-unix-realloc-boundary]`: Unix reallocf forwarding in
  `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-unix-page-aligned-boundary]`: Unix valloc/vfree and
  pvalloc forwarding in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]`: Unix aligned,
  memalign, and POSIX memalign forwarding in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-unix-cfree-boundary]`: Unix cfree forwarding in
  `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]`: Unix
  reallocarray/reallocarr forwarding in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-wasi-libc-boundary]`: WASI `__libc_*` allocation
  forwarding shims in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-linux-libc-boundary]`: Linux `__libc_*`
  malloc/calloc/realloc/free/cfree forwarding shims in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-linux-libc-aligned-boundary]`: Linux aligned
  `__libc_*` and `__posix_memalign` forwarding shims in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-allocation-query-boundary]`: malloc size,
  usable-size, and good-size compatibility queries in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-cxx-delete-boundary]`: scalar C++ delete
  compatibility symbols and sized helper shims in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-cxx-array-delete-boundary]`: array C++ delete
  compatibility symbols and sized array helper shims in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]`,
  `SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]`,
  `SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]`, and
  `SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]`: aligned C++ delete,
  pointer-width-specific new, and inline operator compatibility symbols in
  `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]`: non-aligned nothrow C++
  compatibility shims in `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-cxx-new-boundary]`: scalar and array C++ new
  compatibility symbols for pointer-width-specific Itanium ABI variants in
  `src/raw/macros.rs`.
- `SAFETY[AUDIT:raw-macro-header-forwarding-boundary]`: header-shaped raw
  forwarding helpers in `src/raw/macros.rs` that call raw function pointers or
  allocator exports without adding Rust ownership guarantees.
- `SAFETY[AUDIT:init-subproc-identity-boundary]`: subproc ID conversion,
  current subproc lookup, and main subproc ID mapping in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-main-subproc-lifecycle-boundary]`: main subproc address
  resolution and process-global main heap/subproc publication in
  `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-create-boundary]`: subproc metadata allocation,
  lock initialization, and registry insertion in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-metadata-destroy-boundary]`: subproc lock teardown
  and metadata free in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-list-unlink-boundary]`: lock-serialized subproc
  list unlinking in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-destroy-loop-boundary]`: subproc heap, arena, and
  page-map destruction ordering in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-registry-teardown-boundary]`: full registry
  teardown during process shutdown in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-public-destroy-boundary]`: public subproc
  destroy/delete entrypoint validation and dispatch in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-prefix-boundary]`: subproc prefix projection in
  `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-main-heap-load-boundary]`: subproc main-heap and
  heap-to-subproc prefix loads in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-main-heap-export-boundary]`: subproc main-heap
  export fallback in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-current-main-heap-boundary]`: current main-heap
  lookup in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-main-heap-predicate-boundary]`: heap and
  thread-heap main predicates in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-heap-visit-boundary]`: subproc heap visitation in
  `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-subproc-stats-count-boundary]`: subproc stats header,
  stats merge, thread sequence, and thread/heap counter updates in
  `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-process-boundary]`: process initialization and teardown in
  `src/allocator/init.rs`, including one-time state, preloading gates,
  allocator init/done callbacks, post-init option activation, and teardown
  ordering.
- `SAFETY[AUDIT:init-thread-identity-boundary]`,
  `SAFETY[AUDIT:init-thread-done-entry-boundary]`,
  `SAFETY[AUDIT:init-thread-done-dispatch-boundary]`,
  `SAFETY[AUDIT:init-thread-init-default-heap-boundary]`,
  `SAFETY[AUDIT:init-thread-done-cleanup-boundary]`,
  `SAFETY[AUDIT:init-thread-done-tls-reset-boundary]`, and
  `SAFETY[AUDIT:init-thread-init-entry-boundary]`: focused thread identity,
  init, done, cleanup, and stat-update boundaries in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-thread-heap-empty-sentinel-boundary]`,
  `SAFETY[AUDIT:init-thread-heap-main-tls-boundary]`,
  `SAFETY[AUDIT:init-thread-heap-main-static-boundary]`,
  `SAFETY[AUDIT:init-thread-default-heap-association-boundary]`,
  `SAFETY[AUDIT:init-thread-cached-heap-association-boundary]`, and
  `SAFETY[AUDIT:init-thread-heap-safe-access-boundary]`: focused default,
  cached, empty, safe-access, and process-main thread-heap TLS association
  boundaries in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-platform-tls-boundary]`: Windows TLS slots, pthread keys,
  and platform TLS setup/teardown in `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-tld-lifecycle-boundary]`: TLD allocation/free, main/non-main
  TLD setup, threadpool flag mutation, and TLD metadata counters in
  `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-options-guarded-boundary]`: thread-heap option and
  guarded-allocation configuration in `src/allocator/init.rs`, including
  clamped option reads and bounded local thread-heap writes.
- `SAFETY[AUDIT:init-random-cpu-boundary]`: CPU feature detection, CPU
  threshold publication, and random seeding/reseeding in
  `src/allocator/init.rs`.
- `SAFETY[AUDIT:init-platform-hooks-boundary]`: hidden init link-name assembly
  in `src/allocator/init.rs` plus process attach/detach constructor and
  destructor hook glue in `src/platform/mod.rs`.
- `SAFETY[AUDIT:options-storage-lock-boundary]`: option descriptor storage,
  initialization, environment reads, and raw option get/set exports in
  `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-libc-file-symbol-boundary]`: platform-gated C
  stdout/stderr symbol access for output callback compatibility checks in
  `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-recurse-guard-boundary]`: diagnostic output recursion
  guard entry/exit through TLS shims in `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-delayed-output-buffer-boundary]`: bounded delayed
  diagnostic output buffering, flushing, and lock discipline in
  `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-stderr-output-boundary]`: primitive stderr forwarding
  after C string pointer validation in `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-output-callback-boundary]`: registered output callback
  lookup, publication, and invocation through typed callback wrappers in
  `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-format-output-boundary]`: bounded diagnostic formatting,
  thread-prefix formatting, and callback forwarding in
  `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-print-output-boundary]`: public option printing through
  static option-name C strings and the selected output callback path in
  `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-warning-error-callback-boundary]`: warning display
  shims and warning/error counter gates in `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-allocation-failure-callback-boundary]`: error callback
  storage and allocation/error failure callback dispatch in
  `src/allocator/options.rs`.
- `SAFETY[AUDIT:options-assertion-boundary]`: assertion failure diagnostics and
  abort shims that must not unwind in `src/allocator/options.rs`.
- `SAFETY[AUDIT:allocation-platform-shim-boundary]`: allocation-local abort,
  pathconf/realpath, and errno platform shims in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-product-entry-boundary]`: calloc, mallocn, and
  checked product allocation entry shims in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-usable-size-entry-boundary]`: usable-size
  allocation exports and good-size wrappers in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-small-dispatch-boundary]`: small allocation dispatch
  in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-thread-heap-zero-boundary]`: thread-heap zero and
  allocation dispatch in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-public-malloc-boundary]`: public malloc and heap
  malloc entrypoints in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-zeroing-dispatch-boundary]`: zalloc entrypoints in
  `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-calloc-dispatch-boundary]`: calloc dispatch in
  `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-layout-request-boundary]`: layout padding, request
  construction, and realloc-size validation in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-realloc-thread-entry-boundary]`: thread-heap realloc
  entrypoints in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-realloc-product-boundary]`: reallocn product paths
  in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-realloc-free-on-failure-boundary]`: reallocf paths
  in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-realloc-zeroing-boundary]`: rezalloc and recalloc
  paths in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-realloc-public-boundary]`: public realloc ABI
  entrypoints in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-string-dup-boundary]`: strdup and strndup
  allocation entry shims in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-realpath-boundary]`: realpath allocation entry shims
  in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-cxx-new-handler-boundary]`: C++ new-handler lookup
  and invocation in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-cxx-new-try-boundary]`: C++ new retry helpers in
  `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-cxx-new-public-boundary]`: public C++ new entrypoints
  in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-cxx-new-product-boundary]`: C++ new_n product paths
  in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-cxx-new-aligned-boundary]`: nothrow and aligned C++
  new paths in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-cxx-new-realloc-boundary]`: C++ new realloc wrappers
  in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:allocation-page-refill-boundary]`: page refill handoff,
  generic fallback allocation, guarded allocation, zeroing, and padding canary
  publication in `src/allocator/alloc.rs`.
- `SAFETY[AUDIT:page-free-list-collect-boundary]`,
  `SAFETY[AUDIT:page-free-list-extend-boundary]`,
  `SAFETY[AUDIT:page-fresh-allocation-boundary]`, and
  `SAFETY[AUDIT:page-queue-search-refill-boundary]`: page-local free-list
  collection, extension, fresh-page publication, and queue search/refill in
  `src/allocator/page.rs`.
- `SAFETY[AUDIT:page-queue-full-transition-boundary]`,
  `SAFETY[AUDIT:page-queue-free-release-boundary]`,
  `SAFETY[AUDIT:page-queue-retire-boundary]`,
  `SAFETY[AUDIT:page-queue-abandon-boundary]`,
  `SAFETY[AUDIT:page-queue-reclaim-boundary]`,
  `SAFETY[AUDIT:page-queue-validation-boundary]`,
  `SAFETY[AUDIT:page-queue-direct-cache-boundary]`,
  `SAFETY[AUDIT:page-queue-link-mutation-boundary]`, and
  `SAFETY[AUDIT:page-queue-append-boundary]`: focused page queue movement,
  full/free/retire/abandon/reclaim, validation, direct-cache, link mutation,
  and append operations in `src/allocator/page.rs`.
- `SAFETY[AUDIT:page-huge-allocation-boundary]`: huge-page selection and
  allocation dispatch in `src/allocator/page.rs`.
- `SAFETY[AUDIT:page-metadata-accessor-boundary]`: page metadata accessors in
  `src/allocator/page.rs`.
- `SAFETY[AUDIT:page-stats-public-boundary]`: stats/bin helpers, public
  C-shaped page helpers, and generic allocation entry shims in
  `src/allocator/page.rs`.
- `SAFETY[AUDIT:arena-slice-geometry-boundary]`,
  `SAFETY[AUDIT:arena-slice-containment-boundary]`, and
  `SAFETY[AUDIT:arena-slice-layout-boundary]`: arena slice geometry,
  containment queries, info-slice sizing, and bitmap layout in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-registry-count-index-boundary]`: arena count/index lookup
  and C ABI registry shims in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-heap-page-table-boundary]`: heap arena-page table lookup
  and bitmap ownership checks in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-registry-start-index-boundary]`: bounded arena iteration
  start-index selection in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-registry-suitability-boundary]`: pinned, requested-arena,
  and NUMA suitability predicates in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-registry-iteration-boundary]`: read-only arena iteration
  through typed subproc table cells in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-registry-destroy-boundary]`: unsafe arena teardown and
  subproc table clearing in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-registry-add-boundary]`: arena registration and atomic
  subproc table publication in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-slice-commit-boundary]`,
  `SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]`,
  `SAFETY[AUDIT:arena-slice-commit-accounting-boundary]`, and
  `SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]`: arena slice commit,
  bitmap mutation, commit accounting, memid publication, and allocation
  search/dispatch in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-initialization-boundary]`: arena metadata
  initialization, guard-page setup, bitmap layout, page metadata table setup,
  and registry insertion in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-reserve-growth-boundary]`: reserve growth, retry
  sizing, reserve-lock discipline, and commit accounting balance in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-os-fallback-boundary]`: OS allocation fallback for
  internal arena allocation shims in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-public-reserve-boundary]`: public reserve/manage APIs
  and OS-memory arena registration in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-huge-reservation-boundary]`: huge OS page reservation
  APIs, NUMA normalization, registration, and release-on-failure handling in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-abandoned-boundary]`: abandoned-page claim/reclaim in
  `src/allocator/arena.rs`. The boundary covers abandoned ownership transfer,
  remote-free collection, and thread-local claim callbacks.
- `SAFETY[AUDIT:arena-abandoned-page-table-boundary]`: heap arena-page table
  lookup, lazy allocation, and publication for abandoned-page search in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-fresh-boundary]`: fresh arena-backed page
  reservation, memid projection, and OS fallback in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-fresh-metadata-boundary]`: fresh page metadata
  placement, initial commit, zeroing checks, and layout counters in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-regular-boundary]`: regular size-class page
  allocation and abandoned-page reuse in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-singleton-boundary]`: singleton page setup for
  over-aligned and large-object allocations in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-free-entry-boundary]`: arena page free raw entry
  conversion into typed all-free witnesses in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-free-validation-boundary]`: arena page free
  ownership, alignment, memid, and bitmap validation in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-free-stats-boundary]`: arena page free stat
  decrements for current-thread, heap, and subproc counters in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-free-release-boundary]`: arena page free page-map,
  bitmap, separated metadata, and backing-slice release in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-abandon-boundary]`: arena page abandon shims,
  abandoned bitmap publication, owner-id updates, heap-list updates, and stat
  accounting in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-reabandon-boundary]`: reabandon shims and
  abandoned-page republication in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-unabandon-entry-boundary]`: unabandon entry
  validation and same-thread ownership checks in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-unabandon-mapped-boundary]`: mapped-abandoned bitmap
  and heap abandoned-count cleanup in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-unabandon-os-list-boundary]`: OS abandoned-list
  unlink and stat reconciliation under the heap abandoned lock in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-heap-page-publication-boundary]`: heap arena-page bitmap
  publication and rollback during page allocation in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-page-ownership-transition-boundary]`: owner-word and
  page-map visibility transitions in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-purge-schedule-boundary]`: delayed purge scheduling and
  purge bitmap publication in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-purge-free-dispatch-boundary]`: arena free dispatch into
  purge scheduling and OS/meta free forwarding in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-purge-direct-boundary]`: direct OS purge/decommit handoff
  and committed-slice bitmap updates in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-purge-range-boundary]`: purge range claiming, single-slice
  fallback, and free bitmap restoration in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-purge-visitor-boundary]`: purge visitor C ABI range/state
  validation in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-purge-expire-policy-boundary]`: per-arena purge expiration
  policy, purge attempt accounting, and bitmap traversal in
  `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-purge-collect-guard-boundary]`: subproc-wide arena
  collection under the global purge guard in `src/allocator/arena.rs`.
- `SAFETY[AUDIT:arena-debug-snapshot-boundary]`: read-only arena diagnostic
  snapshots in `src/allocator/arena/debug.rs`.
- `SAFETY[AUDIT:arena-debug-page-state-boundary]`: page-state classification
  for arena diagnostics in `src/allocator/arena/debug.rs`.
- `SAFETY[AUDIT:arena-debug-buffer-boundary]`: bounded stack-buffer output and
  ANSI diagnostic line rendering in `src/allocator/arena/debug.rs`.
- `SAFETY[AUDIT:arena-debug-chunk-render-boundary]`: chunk and page bitfield
  rendering in `src/allocator/arena/debug.rs`.
- `SAFETY[AUDIT:arena-debug-registry-walk-boundary]`: current subproc arena
  registry walking and C-shaped debug printer exports in
  `src/allocator/arena/debug.rs`.
- `SAFETY[AUDIT:page-count-accessors]` and
  `SAFETY[AUDIT:page-queue-count-accessors]`: page metadata counters and
  page-queue counter/sentinel helpers in `src/allocator/page/counts.rs`.
- `SAFETY[AUDIT:internal-bitmap-simd-boundary]`: optional AVX2 bitmap helper
  intrinsics in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-metadata-boundary]`: bitmap sizing,
  initialization, and chunk-count metadata in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-chunkmap-boundary]`: chunkmap and chunk-bin
  bookkeeping in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-claim-boundary]`: regular bitmap first-fit
  claim traversal and typed claim callback handoff in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-range-mutation-boundary]`: regular bitmap
  set/clear chunk-range operations in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-range-query-boundary]`: regular bitmap
  test/scan/popcount chunk-range operations in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-traversal-boundary]`: set/range traversal
  callback shims in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-binned-boundary]`: binned-bitmap
  set/clear/test operations in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-binned-claim-boundary]`: binned-bitmap
  find-and-clear claim paths in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-bitmap-test-fixtures]`: local bitmap unit-test raw
  callback fixtures in `src/internal/bitmap.rs`.
- `SAFETY[AUDIT:internal-runtime-shim-boundary]`: assertion shims in
  `src/internal/mod.rs`.
- `SAFETY[AUDIT:internal-c-abi-shim-boundary]`: translated internal helper
  declarations in `src/internal/mod.rs`.
- `SAFETY[AUDIT:internal-cpu-memory-boundary]`: copy/fill/zero helpers in
  `src/internal/mod.rs`.
- `SAFETY[AUDIT:internal-thread-heap-shim-boundary]`: thread-heap and guarded
  allocation helper projections in `src/internal/mod.rs`.
- `SAFETY[AUDIT:internal-direct-bin-boundary]`,
  `SAFETY[AUDIT:internal-page-map-lookup-boundary]`,
  `SAFETY[AUDIT:internal-page-size-count-boundary]`,
  `SAFETY[AUDIT:internal-page-owner-heap-boundary]`,
  `SAFETY[AUDIT:internal-page-location-boundary]`, and
  `SAFETY[AUDIT:internal-page-flag-owner-boundary]`: focused direct-bin,
  page-map lookup, page size/count, owner/heap, location, and flag/owner
  helpers in `src/internal/mod.rs`.
- `SAFETY[AUDIT:internal-thread-free-boundary]`: thread-free ownership-word
  helpers in `src/internal/mod.rs`.
- `SAFETY[AUDIT:internal-block-link-boundary]`: encoded free-list link helpers
  in `src/internal/mod.rs`.
- `SAFETY[AUDIT:unix-file-syscall-boundary]`,
  `SAFETY[AUDIT:unix-mmap-primitive-boundary]`,
  `SAFETY[AUDIT:unix-memory-alloc-boundary]`,
  `SAFETY[AUDIT:unix-memory-commit-boundary]`,
  `SAFETY[AUDIT:unix-process-info-boundary]`,
  `SAFETY[AUDIT:unix-output-env-boundary]`,
  `SAFETY[AUDIT:unix-random-boundary]`,
  `SAFETY[AUDIT:unix-thread-tls-boundary]`,
  `SAFETY[AUDIT:unix-huge-page-boundary]`,
  `SAFETY[AUDIT:unix-numa-boundary]`, and
  `SAFETY[AUDIT:unix-ffi-import-boundary]`: Unix primitive backend slices in
  `src/platform/unix.rs`.
- `SAFETY[AUDIT:windows-virtual-memory-import-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-config-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-commit-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-decommit-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-release-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-reset-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-protect-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-reserve-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-retry-boundary]`,
  `SAFETY[AUDIT:windows-virtual-memory-allocation-export-boundary]`,
  `SAFETY[AUDIT:windows-process-info-boundary]`,
  `SAFETY[AUDIT:windows-output-env-boundary]`,
  `SAFETY[AUDIT:windows-fls-api-boundary]`,
  `SAFETY[AUDIT:windows-dll-lifecycle-boundary]`,
  `SAFETY[AUDIT:windows-thread-yield-boundary]`,
  `SAFETY[AUDIT:windows-dll-entry-boundary]`,
  `SAFETY[AUDIT:windows-allocator-lifecycle-boundary]`,
  `SAFETY[AUDIT:windows-fls-thread-boundary]`,
  `SAFETY[AUDIT:windows-teb-threadpool-boundary]`,
  `SAFETY[AUDIT:windows-large-page-boundary]`,
  `SAFETY[AUDIT:windows-huge-page-allocation-boundary]`,
  `SAFETY[AUDIT:windows-numa-boundary]`,
  `SAFETY[AUDIT:windows-etw-boundary]`,
  `SAFETY[AUDIT:windows-random-boundary]`, and
  `SAFETY[AUDIT:windows-dynamic-import-boundary]`: Windows primitive backend
  and translated ETW helper slices in `src/platform/windows.rs`.
- `SAFETY[AUDIT:os-memory-public-export-boundary]` and
  `SAFETY[AUDIT:os-memory-retained-wrapper-boundary]`: public OS allocation
  wrappers and retained C ABI wrapper statics in `src/platform/os.rs`.
- `SAFETY[AUDIT:macos-zone-allocation-boundary]` and
  `SAFETY[AUDIT:macos-zone-foreign-forwarding-boundary]`: mimalloc zone
  allocation trampolines and foreign-zone callback forwarding in
  `src/platform/macos.rs`.
- `SAFETY[AUDIT:internal-atomic-raw-view-boundary]`,
- `SAFETY[AUDIT:internal-atomic-load-store-boundary]`,
- `SAFETY[AUDIT:internal-atomic-fetch-boundary]`,
- `SAFETY[AUDIT:internal-atomic-integer-fetch-boundary]`,
- `SAFETY[AUDIT:internal-atomic-cas-boundary]`,
- `SAFETY[AUDIT:internal-atomic-exchange-boundary]`,
- `SAFETY[AUDIT:internal-atomic-max-boundary]`,
- `SAFETY[AUDIT:internal-atomic-volatile-delta-boundary]`,
  `SAFETY[AUDIT:internal-atomic-pointer-boundary]`,
  `SAFETY[AUDIT:internal-atomic-guard-once-boundary]`, and
  `SAFETY[AUDIT:internal-atomic-pthread-lock-boundary]`,
  `SAFETY[AUDIT:internal-atomic-fallback-lock-boundary]`: internal atomic
  raw-view, scalar operation, pointer, guard/once, and lock facade slices in
  `src/internal/atomic.rs`.
- `SAFETY[AUDIT:unsafe-callback-guard-boundary]`: unsafe-adjacent callback guards.
  Unsafe guard construction registers a trampoline with a borrowed callback
  context, stores the previous registration, restores that previous registration on drop,
  restores the process-global snapshot, and catches Rust
  panics before callback ABI return in `std` and test builds. Callers must
  prevent concurrent registration changes; no concurrent or reentrant entry may access the same
  borrowed callback.
- `SAFETY[AUDIT:unsafe-error-callback-test-fixtures]`,
  `SAFETY[AUDIT:unsafe-deferred-callback-test-fixtures]`, and
  `SAFETY[AUDIT:unsafe-callback-address-test-fixtures]`: callback tests
  serialize process-global registration, use synchronous local thread-heap
  fixtures, and copy callback function pointer options only for equality
  checks. Output callbacks remain raw until output recursion and diagnostic
  allocation semantics are designed.
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
