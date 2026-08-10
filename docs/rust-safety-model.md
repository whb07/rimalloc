# Rust Safety Model

This crate separates Rust-safe ownership from C-shaped allocator compatibility.
The safe facade is intentionally narrow: it exposes only APIs whose ownership,
initialization, lifetime, and error behavior can be enforced without caller
discipline. Raw C compatibility stays under `mechanical_mimalloc::raw`, and
borrowed callback guards stay under the Tier U `unsafe_callbacks` namespace.
Rust-facing naming, constructor, accessor, `Copy`/`Clone`, unsafe-comment, and
hot-path wrapper conventions are defined in `docs/rust-style.md`.

## Safe Ownership And Lifetimes

`MiMalloc` is the only safe global allocator facade. It is a zero-sized
`GlobalAlloc` implementation; Rust's allocator contract supplies valid layouts
and live allocation pointers to the unsafe trait methods.

`Allocation` and `HeapAllocation<'heap>` own byte allocations and free them
exactly once on drop. Resizing preserves ownership on failure by returning typed
errors that carry the original allocation. Zero-sized allocations are modeled as
aligned sentinels and do not call into allocator allocation paths.

`Heap`, `HeapRef<'heap>`, `MainHeap`, `HeapAllocation<'heap>`, and
`InitializedHeapAllocation<'heap>` encode heap ownership with lifetimes. Safe
heap allocations borrow the heap witness, so safe code cannot delete an owned
heap while heap-tied allocations are live. `MainHeap` is a borrowed witness for
the process-global main heap; dropping the handle does not delete that heap.

The safe facade currently owns byte storage only, not initialized Rust objects.
Object ownership APIs such as `MiBox<T>` and `HeapBox<'heap, T>` are not part
of the current public API.
`MiBox<T>` and `HeapBox<'heap, T>` are deferred for the current release line in
`docs/object-ownership.md`; they require a separate object lifetime design for
allocation failure, initialization, `Drop`, destructor panic behavior,
zero-sized values, unsized values, and heap-tied ownership.

## Initialization

Uninitialized memory is never exposed as `&[u8]` or `&mut [u8]`. `Allocation`
may expose raw pointer escape hatches for unsafe initialization, but safe byte
slices are available only through initialized wrappers such as
`InitializedAllocation` and `InitializedHeapAllocation`.

Code that fills an uninitialized allocation through raw pointers must complete
initialization for every byte before calling `Allocation::assume_initialized` or
`HeapAllocation::assume_initialized`. That transition is unsafe because the
caller, not the type system, proves the bytes are initialized.

## Process-Global Callbacks

`unsafe_callbacks::ErrorCallbackGuard` and
`unsafe_callbacks::DeferredFreeCallbackGuard` are Tier U unsafe Rust APIs, not
Tier S safe facade APIs. They register process-global mimalloc callbacks with a
borrowed `FnMut` context, restore the previous registration on drop, and catch
Rust panics before returning through the callback ABI in `std` and test builds.

Their constructors are unsafe because mimalloc can invoke process-global
callbacks from allocator activity outside the registering scope. Callers must
serialize registration, restoration, and callback entry for the borrowed
closure, prevent concurrent or reentrant access to the same `FnMut`, and drop
nested guards in LIFO order.

## Raw Pointer Boundaries

The raw Rust facade mirrors the C API. It uses raw pointers, C null/error
conventions, public C layouts, callbacks, and symbol names. Raw callers uphold
allocation origin, pointer validity, layout, alignment, lifetime, nullability,
threading, callback, and unwind obligations.

C ABI exports and allocator hot paths parse raw inputs at narrow boundaries and
then move through crate-private typed wrappers where possible. Safe wrappers do
not expose allocator metadata pointers or raw deletion rights. Internal
hot-path wrappers are expected to compile away; they must not add allocation,
formatting, panic, lock, or syscall behavior to allocator critical paths.

## Unsafe Evidence

`docs/unsafe-audit.md` owns the unsafe-code policy. Each unsafe operation has a
local `// SAFETY:` proof, while behavior, concurrency, provenance, ABI, and
code-generation claims are covered by the focused test or tool appropriate to
the risk. The project does not maintain a parallel operation-ID ledger.

Allocator hot paths are guarded by the default `no_std` build, allocation
boundary tests, and release evidence. New strict internal wrappers on hot paths
require release assembly or benchmark evidence for free-list pop, local free,
remote free, small page lookup, and any changed path.

## Strict Internal Types

Strict internal types live under `src/types/` and are crate-private. They carry
allocator invariants such as non-null metadata pointers, free-list ownership,
page states, heap ownership, memid interpretation, layout requests, and atomic
metadata access.

These wrappers are not public Rust API, raw facade API, or C ABI. Hot wrappers
use transparent representation, inline constructors/accessors, and layout/niche
tests so the type-level invariants do not change C layout or hot-path code
shape. If a wrapper introduces measurable hot-path cost, the change must be
justified with evidence or redesigned.
