# Memory Ordering Policy

This allocator mirrors mimalloc's C atomic contract while keeping Rust's
atomic casts behind crate-private typed wrappers. Memory ordering changes are
performance-sensitive and correctness-sensitive: every acquire, release,
acqrel, seq-cst, or relaxed operation must have an allocator invariant behind
it.

## Atomic Wrapper Boundary

Raw field-to-atomic casts are centralized in `src/core_types/atomic_wrappers.rs`.
Allocator code should use the typed wrappers instead of spelling
`AtomicUsize::from_ptr`, `AtomicPtr::from_ptr`, or direct pointer casts at call
sites. This keeps provenance, alignment, and field-domain assumptions visible
at the boundary where C layout is reinterpreted as Rust atomics.

`src/atomic.rs` remains the C ABI compatibility layer for the upstream
`mi_atomic_*` API. New internal code should prefer the typed wrappers when it
is operating on allocator metadata fields.

## Ordering Vocabulary

- `Relaxed` is allowed for telemetry, counters, timestamps, retry hints, and
  state bits where no memory publication or consumption is attached.
- `Release` publishes allocator metadata or ownership transitions that another
  thread may later observe.
- `Acquire` consumes metadata or ownership published by a release operation.
- `AcqRel` is used for read-modify-write transitions that both consume old
  state and publish new state.
- `SeqCst` is reserved for C API parity where upstream exposes that explicit
  ordering; it is not the default policy for allocator internals.

## Thread-Free Lists

Remote frees publish blocks to a page's thread-free list with release semantics.
The owning thread consumes that list with acquire or acqrel operations before it
links the blocks into local allocator structures. The owner bit is part of the
same packed domain, so ownership changes use the typed `AtomicThreadFree`
helpers rather than untyped integer atomics.

Relaxed loads of the packed thread-free word are only acceptable for retry
loops or inspections that do not dereference newly published blocks. Taking or
claiming ownership must go through the acquire/acqrel helper paths that preserve
owner bits.

## Page Ownership And Abandonment

Page owner and flag transitions are represented through `AtomicPageThreadId`.
Publishing abandonment or ownership transfer uses release or acqrel operations;
claiming or observing ownership before touching page metadata uses acquire or
acqrel operations. Flag updates that participate in ownership transitions must
not be downgraded to relaxed bit operations.

## Page Map And Metadata Publication

Page-map submaps and heap/page metadata pointers are published with release
semantics and looked up with acquire semantics. A thread that observes a
non-null published pointer may then dereference the associated metadata only
after the acquire load or successful acqrel compare-exchange that installed or
claimed it.

## Deferred Free Callback

The deferred-free function pointer and its argument are a publication pair.
The setter stores the argument and callback with release ordering; the caller
loads them with acquire ordering before invocation. This preserves callback
data visibility without requiring a global lock in the hot path.

## Arena State

Arena purge deadlines, delayed purge fields, in-use counts, and sub-process
arena counts use typed scalar wrappers. Release stores and acqrel
compare-exchanges publish state transitions that other threads may act on.
Relaxed reads remain appropriate for non-authoritative timestamps, counters,
or retry decisions where a stale value merely causes another scan or CAS.

## Thread Locals

Linux x86_64 TLS cells are not Rust atomics themselves, but the dynamic TLS
slot list and initialization state are shared allocator metadata:

- `mi_thread_locals_initialized` uses acqrel compare-exchange on first
  initialization and acquire on failed contenders.
- `mi_thread_locals_free` is stored with release, loaded with acquire, and
  drained with acqrel swap.
- `mi_thread_locals_version` is a generation counter guarded by the TLS lock,
  so relaxed loads/stores are sufficient.

## Locks And Once

Allocator locks publish protected writes on unlock and consume them on lock.
The atomic once implementation uses acquire on observation, acqrel on entry,
and release on completion, matching the upstream one-time initialization
contract.

## Stats And Diagnostics

Stats, error counters, benchmark counters, and test-only accounting may use
relaxed atomics when they do not publish allocator metadata and when stale
values do not change memory safety. If a diagnostic counter starts controlling
allocator behavior, its ordering must be re-audited.

## Verification

Ordering-sensitive changes should run:

```sh
scripts/check-atomic-ordering-audit.py
scripts/check-layout.sh
cargo test --quiet
```

`tests/release_fast_path_audit.rs` also checks that selected metadata fields
continue to use the typed wrappers and that known hot-path atomic shapes do not
silently drift.
