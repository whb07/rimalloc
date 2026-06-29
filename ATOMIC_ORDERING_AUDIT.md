# Atomic Ordering Audit

Scope: Rust sources in `src/` that use `core::sync::atomic` or `std::sync::atomic`
directly, compared with mimalloc 3.3.2's `include/mimalloc/atomic.h` and the
corresponding C call sites.

The Rust port uses the same or stronger orderings as the C implementation for the
supported default matrix:

- `src/atomic.rs`: mimalloc atomic helper wrappers preserve C macro semantics.
  Relaxed loads/stores/RMWs stay `Relaxed`; acquire loads stay `Acquire`;
  release stores stay `Release`; acq-rel RMW/CAS success paths stay `AcqRel`;
  CAS failure paths use `Acquire` where C uses acquire and `Relaxed` where C uses
  relaxed. The lock and once helpers match `atomic.h`/`libc.c`: lock acquire is
  CAS `AcqRel` with `Acquire` failure, release is `Release`, once entry loads
  `Acquire`, claims with CAS `AcqRel`, and publishes completion with `Release`.
- `src/bench_diagnostics.rs`: diagnostic counters use relaxed atomics because
  they are process-local measurement aids and do not publish allocator metadata.
- `src/arena.rs`: arena publication and lookup orderings match C arena/page
  ownership paths. Published `arena_pages` pointers use `Release` stores and
  `Acquire` loads; abandoned counters and local heap counts stay relaxed as in C;
  purge guards use the audited `mi_atomic_guard` helper.
- `src/arena_meta.rs`: meta-page stack publication uses `Acquire` loads,
  `Release` next-pointer publication, and `AcqRel` CAS for the global head, as in
  `arena-meta.c`.
- `src/alloc.rs`: the cached `PATH_MAX` value follows `alloc.c`: acquire loads
  observe the published value and acq-rel compare-exchange publishes the first
  successful discovery.
- `src/alloc_override.rs`: override bootstrap allocation tracking uses atomic
  counters to coordinate C/C++ interposition tests without publishing allocator
  metadata.
- `src/bitmap.rs`: bitmap bit updates mirror `bitmap.c`: set/clear transitions
  use acq-rel RMW/CAS, state queries that synchronize with those transitions use
  acquire loads, and scan-only/counting paths retain relaxed loads.
- `src/core_types/atomic_wrappers.rs`: typed allocator-field wrappers centralize
  pointer-to-atomic reinterpretation and expose the fixed acquire/release/acq-rel
  helper operations used by page ownership, thread-free lists, arena state, and
  page-map publication.
- `src/core_types/atomic_wrappers/concurrency_models.rs`: loom models exercise
  the ordering contracts for remote free-list and ownership transitions.
- `src/core_types/atomic_wrappers/tests.rs`: unit tests assert the wrapper-level
  acquire/release/acq-rel operations used by allocator metadata fields.
- `src/heap.rs`: heap sequence/count updates and arena-page cleanup retain the C
  relaxed ordering because external synchronization or ownership already protects
  the structural mutation.
- `src/init.rs`: subprocess thread/heap counters retain the C relaxed ordering;
  one-time process/default initialization uses acquire/acq-rel/release ordering to
  publish state at least as strongly as the C once/lock helpers.
- `src/options.rs`: delayed output, registered output/error callback pointers,
  callback args, and throttle counters use acquire/release/acq-rel orderings that
  match `options.c`.
- `src/os.rs`: aligned hint allocation, huge-page range reservation, and NUMA
  count publication mirror `os.c`: hint/range CAS paths are acq-rel, NUMA count
  publication is release/acquire, and read-only fast paths that are relaxed in C
  remain relaxed.
- `src/page_map.rs`: page-map commit bits and submap publication preserve the C
  acquire/release/acq-rel discipline for publishing page map entries.
- `src/prim/unix.rs`: large-page retry state and getrandom fallback state match
  `prim/unix/prim.c` acquire/release/acq-rel usage; reset advice uses relaxed
  load plus release fallback publication as in C.
- `src/prim/windows.rs`: Windows primitive globals mirror
  `prim/windows/prim.c`: frequency and large-page/huge-page state are published
  with release and read with acquire, with relaxed-free operation only where C has
  no synchronization dependency.
- `src/stats.rs`: clock calibration and process-start atomics are Rust-only
  monotonic caches. They use relaxed compare-exchange/load because callers do not
  publish allocator metadata through these values; allocator stats counters use
  the audited mimalloc atomic helpers.
- `src/safe.rs`: tests use atomics to count callback invocations and panic
  containment effects; they do not publish allocator metadata.
- `src/threadlocal.rs`: thread-local slot free-list publication uses release
  stores and acquire loads; version counters are relaxed because they are advisory
  freshness checks, matching their non-owning role.

Validation:

- `scripts/check-atomic-ordering-audit.py` verifies every `src/` file with direct
  `Ordering::` usage is listed here, and checks the central C-to-Rust helper
  ordering mappings in `src/atomic.rs`.
- `cargo test` exercises the cross-thread allocation/free, abandoned-page,
  process/thread lifecycle, callback reentrancy, and stress workloads that depend
  on these orderings.
- `scripts/check-behavioral-parity.py` compares the same concurrency and lifecycle
  workloads against C mimalloc.
