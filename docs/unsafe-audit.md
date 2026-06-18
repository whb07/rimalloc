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
  slice boundaries, and OS out-parameters.
- `threadlocal/theap`: thread-local heap pointer lifetime, abandonment, reclaim,
  and cross-thread publication.
- `atomic`: acquire/release/relaxed ordering around free lists, ownership, locks,
  and once initialization.
- `core_types`: crate-private witnesses for pointer domains, layout requests,
  free-list ownership, page states, memid views, and atomic metadata fields.
- `track`: Valgrind/ASan hooks and feature-exclusive instrumentation.
- `raw ABI`: C callbacks, null/error conventions, exported symbols, and panic
  boundaries.

## Boundary Audit Tags

- `SAFETY[AUDIT:global-alloc]`: the `MiMalloc` `GlobalAlloc` facade. Callers
  provide valid non-zero `Layout` values and live allocation pointers according
  to Rust's allocator contract. The callee dispatches to normal or aligned
  mimalloc paths, returns null on failure, preserves reallocation contents on
  success, and does not unwind intentionally.
- `SAFETY[AUDIT:owned-allocation]`: the safe `Allocation` wrapper. The wrapper
  owns one allocation, frees it exactly once on drop, treats zero-sized
  allocations as aligned sentinels, exposes raw pointers without transferring
  free rights, and never exposes safe references to uninitialized bytes.
- `SAFETY[AUDIT:ffi-export]`: exported C ABI functions and aliases. Callers
  must uphold the C API pointer, layout, ownership, callback, and threading
  rules. Exported functions must not unwind across the C boundary and must stay
  classified by the FFI/export boundary checks.
- `SAFETY[AUDIT:raw-callback]`: raw callback entrypoints and callback storage.
  Callers must provide ABI-compatible function pointers and user data that stay
  valid for the documented call duration. Safe callback wrappers are deferred.
- `SAFETY[AUDIT:panic-boundary]`: no-std panic/personality shims and aborting
  assertion paths. These paths must not allocate or unwind.
- `SAFETY[AUDIT:heap-wrapper]`: reserved for a future safe heap wrapper. No
  safe heap wrapper is currently exposed.

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
- `Allocation` is the only owned safe allocation wrapper. It has raw pointer
  escape hatches, no safe byte-slice access, no raw ownership-transfer API, and
  tagged `Send`/`Sync` proofs under `SAFETY[AUDIT:owned-allocation]`.
- Future safe heap, stats, options, process-info, and callback wrappers are
  deferred until their ownership, aliasing, lifetime, initialization,
  recursion, concurrency, platform, and unwind behavior are designed.
- Raw pointer escape hatches from safe types must document invalidation, null or
  sentinel behavior, caller free restrictions, use-after-move/drop behavior,
  aliasing, and thread restrictions.

## Policy

- Prefer local unsafe blocks with nearby safety reasoning over broad
  `unsafe_op_in_unsafe_fn` allowances.
- Do not add module-level `allow(unsafe_op_in_unsafe_fn)`.
- Any new unsafe wrapper must document caller obligations.
- Any new safe wrapper around raw allocator state must explain how it enforces
  ownership, lifetime, and aliasing invariants.
- `scripts/check-safe-boundary.py` enforces that the safe facade has no public
  unsafe Rust items and that boundary unsafe blocks in the safe facade carry
  stable audit tags.
