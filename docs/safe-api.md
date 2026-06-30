# Safe Rust API Policy

The safe Rust API is intentionally narrow. This crate is primarily a
mimalloc-compatible C ABI allocator artifact, with Rust wrappers that encode
the allocator ownership, initialization, heap lifetime, and snapshot rules that
can be upheld without caller discipline.

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

## Adding Safe Wrappers

New safe wrappers belong in the crate-root safe facade, with implementation in
`src/safe.rs` and re-exports from `src/lib.rs` only after their invariants are
documented and tested. They must be added to `docs/api-inventory.md`, covered by
safe facade tests, and reviewed in `docs/unsafe-audit.md` if they contain unsafe
blocks or unsafe trait impls.

Safe wrappers must not be added under `raw`. The `raw` facade remains the
C-shaped unsafe compatibility surface, and implementation modules remain
private unless a future public API design explicitly exposes them with
inventory and compatibility review.
Safe facade contact with allocator internals is file-specific and checked by
`scripts/check-layer-boundaries.py`.

## Current Safe Surface

- `MiMalloc`: `GlobalAlloc` facade backed by the allocator.
- `Allocation`: owned byte allocation wrapper that frees on drop and supports
  ownership-preserving resize.
- `InitializedAllocation`: initialized owned byte allocation wrapper that can
  expose `&[u8]` and `&mut [u8]`.
- `Heap`, `HeapRef`, `MainHeap`, `HeapAllocation`, and
  `InitializedHeapAllocation`: heap ownership and heap-tied allocation wrappers
  that prevent deleting an owned heap while safe heap allocations are live.
  `MainHeap` is a borrowed witness for the process-global main heap; dropping
  the handle does not delete the heap, so its allocations are tied to
  `HeapRef<'static>` rather than to the temporary handle borrow.
- `GlobalOptions` and `OptionKey`: supported process-global allocator options
  with typed errors for invalid or immutable keys.
- `StatsSnapshot`, `StatCount`, and `StatTotal`: owned non-printing statistics
  snapshots and typed statistic values.
- `ProcessInfo`: typed process allocator information. Size totals are `None`
  when the raw process-info API reports zero; `page_faults()` returns a direct
  count because zero is a valid observed count.
- `AllocError`, `LayoutError`, `TryAllocError`, `ResizeError`,
  `ResizeErrorKind`, `HeapError`, `OptionError`, and `StatsError`: typed
  failure states for the safe API.

Raw heap, stats, option, process-info, and callback functions remain available
under `raw` for C-shaped integration and parity testing. Safe wrappers do not
expose raw allocator metadata pointers or transfer raw deletion rights.
Borrowed `FnMut` callback guards are available under `unsafe_callbacks` for
callers that can uphold the process-global synchronization, lifetime,
reentrancy, and allocator-entry obligations. They restore previous callback
registrations on drop and catch Rust panics before returning through the
callback ABI in `std` and test builds, but they are not Tier S safe facade APIs.
Nested guards must be dropped in LIFO order, and concurrent registration,
restoration, or callback entry remains the caller's obligation.
They are Tier U unsafe Rust convenience APIs and remain separate from this
current safe surface.

## Pre-release Migration Notes

The pre-release `Allocation::into_initialized` and
`InitializedAllocation::into_initialized` methods were removed because they did
not perform a meaningful runtime conversion. Code that allocates zeroed memory
should use `Allocation::zeroed`, `Allocation::zeroed_with_size_align`, or the
matching `InitializedAllocation` constructors, which return
`InitializedAllocation` directly. Code that fills an uninitialized `Allocation`
through raw pointers must keep that transition explicit with unsafe
`Allocation::assume_initialized` or `HeapAllocation::assume_initialized` after
every byte has been initialized.

## Future Object Ownership APIs

The current allocation wrappers own bytes, not Rust objects. They must not grow
constructors that initialize `T`, run `Drop`, or otherwise act like `Box<T>`.
Object-level ownership needs separate types, such as `MiBox<T>` for the main
allocator and `HeapBox<'heap, T>` for heap-tied ownership, so byte allocation
layout, initialization, and resize APIs stay distinct from object lifetime
semantics.

Before adding object ownership APIs, the design must specify construction
failure behavior, destructor and panic behavior, allocator/heap ownership, and
threading traits. It must also add tests for successful construction and drop,
constructor panic cleanup, destructor panic behavior, heap-tied lifetime
enforcement, and deallocation through the owning allocator or heap.

## Naming Conventions

- Rust-facing names use idiomatic Rust names: `MiMalloc`, `Allocation`, `Heap`,
  `StatsSnapshot`, `GlobalOptions`, and similar.
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
- `Heap`, `HeapRef`, `MainHeap`, `HeapAllocation`, and
  `InitializedHeapAllocation` are intentionally not `Send` or `Sync`. They
  carry zero-sized marker fields because safe heap operations use mimalloc
  heap/thread-heap metadata that is not modeled as a thread-safe Rust handle.
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
| Resize layout failure or allocation failure | `ResizeError` preserving the original allocation |
| Convenience constructor failure after layout validation | `TryAllocError::Alloc` |
| Heap creation or unsupported heap operation | `HeapError` |
| Invalid, immutable, or unsupported option | `OptionError` |
| Unavailable stats or too-small JSON buffer | `StatsError` |
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
