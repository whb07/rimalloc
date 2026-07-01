# Safe Object Ownership Design Note

Safe object ownership APIs are deferred for the current release line. The safe
facade owns byte allocations through `Allocation`, `InitializedAllocation`,
`HeapAllocation<'heap>`, and `InitializedHeapAllocation<'heap>`; it does not
currently own initialized Rust objects or run arbitrary `Drop` implementations.

This is a release-scoped decision, not a permanent rejection. Adding
`MiBox<T>` or `HeapBox<'heap, T>` changes the safe API contract from byte
storage to object lifetime management, and must land with the full design and
tests below.

## Current Decision

`MiBox<T>` and `HeapBox<'heap, T>` are out of scope before the next release.
The current release checklist rejects object constructors, object destructors,
or public `MiBox`/`HeapBox` names unless this design is completed and the API is
implemented with tests.

The main reasons are:

- Constructor failure must not drop or leak a caller's value unexpectedly.
- Constructor panic cleanup must deallocate raw storage without observing an
  uninitialized `T`.
- `Drop` must run exactly once and deallocate through the allocator or heap that
  created the storage, including unwind behavior when `T::drop` panics.
- Heap-tied object ownership must preserve the same heap lifetime guarantees as
  `HeapAllocation<'heap>`.
- Zero-sized values, dynamically sized values, raw conversions, and auto traits
  need explicit contracts before they become public SemVer surface.

## Future `MiBox<T>` Shape

The first acceptable object owner should be a sized `MiBox<T>` backed by the
process allocator:

- `try_new(value: T) -> Result<MiBox<T>, (AllocError, T)>` allocates storage
  before moving `value` into it, and returns the value unchanged on allocation
  failure.
- `try_new_with(init: impl FnOnce() -> T) -> Result<MiBox<T>, AllocError>`
  allocates first, then runs `init`; if `init` panics, the raw storage is
  deallocated and the panic continues.
- Construction writes with `ptr::write` into properly aligned storage produced
  from `Layout::new::<T>()`.
- `Drop` runs `ptr::drop_in_place` for initialized storage and then deallocates
  through `MiMalloc`. The implementation must include a cleanup strategy for
  unwinding `T::drop` so storage is not silently leaked in `std` builds.
- Zero-sized `T` uses a dangling aligned pointer, never calls the allocator, and
  still runs `T::drop`.
- `MiBox<T>` should not expose byte-resize APIs. Object relocation is a
  different contract from allocation resize.
- `MiBox<T>` may implement `Deref<Target = T>` and `DerefMut` once aliasing and
  initialization invariants are represented privately.
- `MiBox<T>` should be `Send` when `T: Send` and `Sync` when `T: Sync`; any
  manual unsafe impl needs an audit entry.

Unsized support is deferred beyond the first object-owner release. A future
`MiBox<[T]>` or trait-object owner needs a separate design for metadata,
initialization length, layout calculation, and coercions.

## Future `HeapBox<'heap, T>` Shape

`HeapBox<'heap, T>` should mirror `MiBox<T>` while tying deallocation to the
heap witness that created it:

- Constructors take `HeapRef<'heap>` or `&'heap Heap` and return
  `HeapBox<'heap, T>`.
- Allocation and deallocation use the heap allocation family, not the process
  allocator.
- The value cannot outlive the heap borrow. Safe code must not be able to drop
  an owned heap while a `HeapBox<'heap, T>` created from it is live.
- `MainHeap` may produce a `'static` heap-tied owner only if it keeps the same
  process-global main-heap semantics already used by `HeapAllocation<'static>`.
- `HeapBox<'heap, T>` should remain neither `Send` nor `Sync` until heap handle
  thread-safety is modeled more precisely.

## Required Tests Before Implementation

Before object ownership APIs are exported:

- Successful construction, dereference, mutation, and drop for `MiBox<T>`.
- Allocation failure preserves and returns the input value for `try_new`.
- Constructor panic cleanup deallocates raw storage without dropping an
  uninitialized value.
- Destructor panic behavior is documented and tested under `std`.
- Zero-sized values run their destructor and do not allocate.
- Heap-tied construction cannot outlive the creating heap borrow.
- Safe code cannot construct `MiBox<T>` or `HeapBox<'heap, T>` from private
  representation fields.
- Raw conversion APIs, if added, have compile-fail and safety-doc coverage.

Until those tests exist, the safe facade remains byte-owner only.
