# Safe Rust API Policy

The safe Rust API is intentionally narrow. This crate is primarily a
mimalloc-compatible C ABI allocator artifact, with a small experimental safe
Rust allocation facade.

## Design Rules

- Safe APIs must not require callers to uphold pointer provenance, heap
  ownership, aliasing, thread-local, or allocator metadata invariants.
- Raw C null/error conventions must be translated into Rust `Result`, `Option`,
  or safe no-op behavior.
- Safe APIs must not panic except for documented programmer errors such as
  invalid arguments already represented by standard Rust constructors.
- Safe APIs must not allocate internally in allocator hot paths or error paths
  unless the operation is explicitly outside allocator operation.
- Safe wrappers around handles use RAII only when RAII matches mimalloc
  semantics and cannot invalidate live allocations unexpectedly.
- Safe wrappers must not expose raw allocator metadata pointers or
  lifetime-sensitive C handles. Raw escape hatches are unsafe unless they return
  non-owning pointers with clearly documented invalidation rules.

## Current Safe Surface

- `MiMalloc`: `GlobalAlloc` facade backed by the allocator.
- `Allocation`: owned byte allocation wrapper that frees on drop.
- `AllocError`, `LayoutError`, `TryAllocError`: allocation and layout error
  types for the owned wrapper.

Safe heap, stats, options, process-info, and callback APIs are not stabilized.
They should remain raw/unsafe until the wrapper can enforce ownership,
lifetime, aliasing, threading, and reentrancy invariants internally.
There is no safe callback registration API, safe callback closure API, or safe
RAII callback guard. Any future safe callback guard must prove whether `Drop`
unregisters the callback or restores the previous registration, how dangling
contexts are prevented, how panics are stopped before the C ABI boundary, and
whether callback bodies may allocate through this allocator.

## Deferred Safe Heap API

No safe `Heap`, `MainHeap`, `HeapRef`, or `HeapAllocation` API is stabilized for
the next release milestone. The raw heap functions remain available only under
the unsafe `raw` facade.

A future safe heap API needs a separate design that proves:

- owned heaps cannot be destroyed while safe allocations tied to that heap are
  live,
- borrowed main-heap handles cannot be deleted,
- safe allocation methods encode the heap/allocation lifetime relationship,
- thread mobility and concurrent access are explicit trait decisions, and
- raw interop methods cannot transfer deletion rights accidentally.

Until that design exists, there is no safe `Heap::new`, heap allocation method,
safe heap `collect`, safe heap destroy, `Heap::as_raw`, or `Heap::from_raw`.

## Deferred Safe Stats, Options, And Process APIs

No safe `StatsSnapshot`, `Options`, `ProcessInfo`, or callback API is
stabilized for the next release milestone. The corresponding C-shaped APIs
remain under the unsafe `raw` facade.

A future safe stats API must return owned numeric snapshots and document
process-wide, heap-local, thread-local, reset, and merge semantics without
returning references into allocator metadata.

A future safe options API must specify initialization order, runtime mutability,
threading, diagnostics, and callback/output behavior. Safe option diagnostics
must avoid allocator recursion in allocator-critical contexts.

A future safe process-info API must return owned numeric values and represent
unsupported platform fields with `Option` or typed errors. Raw out-pointer
behavior remains under `raw`.

## Naming Conventions

- Rust-facing names use idiomatic Rust names: `MiMalloc`, `Allocation`, `Heap`,
  `StatsSnapshot`, `Options`, and similar.
- C names remain under `raw` or the exported C ABI.
- Fallible methods use `try_` when there is also an infallible variant or when
  Rust convention benefits from the prefix.
- Unsafe ownership-transfer APIs use `from_raw`, `from_raw_unchecked`,
  `as_raw`, and `into_raw` only with explicit safety and invalidation docs.
- C null/error behavior should not leak into safe method names or return types.

## Trait Policy

Implement `Debug`, `Default`, `Copy`, `Clone`, `Eq`, `Hash`, `Send`, and `Sync`
only where the semantics are correct for the safe abstraction.

- `MiMalloc` is a zero-sized global allocator facade. It implements
  `GlobalAlloc`, derives `Copy`, `Clone`, `Debug`, and `Default`, and relies on
  auto `Send`/`Sync` traits from having no state.
- `Allocation` owns memory and releases it on drop. It implements `Debug`, has
  manual `Send`/`Sync` impls because safe shared access exposes no references
  into allocation contents, and does not implement `Copy` or `Clone`.
- Error/status types are plain value types and may implement `Copy`, `Clone`,
  `Debug`, `Eq`, and `PartialEq`.
- Manual unsafe `Send` or `Sync` impls require a `SAFETY:` comment at the impl
  site and a proof in `docs/unsafe-audit.md`.
- Handle wrappers should not implement `Deref` unless the deref target
  semantics are obvious, safe, and do not expose allocator metadata aliases.

## Error Mapping

Safe APIs translate raw C failure conventions at the safe boundary:

| Raw condition | Safe Rust result |
| --- | --- |
| Null from an allocation request with a valid `Layout` | `AllocError` |
| Invalid size/alignment convenience arguments | `LayoutError` |
| Convenience constructor failure after layout validation | `TryAllocError::Alloc` |
| Unsupported platform or feature for a future safe API | a typed `UnsupportedError` or equivalent, if that API is added |
| Raw errno-style behavior | remains under `raw` until wrapped by a typed safe API |

## Feature Expectations

Safe API behavior should remain understandable under:

- default features,
- `std`,
- `secure`,
- `guarded`,
- `track-valgrind`,
- `track-asan`.

Feature-specific hardening or tracking may affect diagnostics and allocator
internals, but safe APIs should keep the same ownership and error contracts
unless a feature explicitly documents otherwise.
