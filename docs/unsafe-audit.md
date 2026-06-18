# Unsafe Audit

Unsafe code in this allocator is expected and must remain documented. Each
unsafe subsystem needs a short proof covering pointer validity, ownership,
alignment, initialization, aliasing, concurrency, and unwind behavior.

## Current Audit Units

- `alloc/free/realloc`: caller pointer ownership, allocation origin, usable-size
  lookups, page metadata lookup, padding checks, and realloc copy bounds.
- `page/arena/os`: page ownership, commit/decommit/reset/purge state, arena
  slice boundaries, and OS out-parameters.
- `threadlocal/theap`: thread-local heap pointer lifetime, abandonment, reclaim,
  and cross-thread publication.
- `atomic`: acquire/release/relaxed ordering around free lists, ownership, locks,
  and once initialization.
- `track`: Valgrind/ASan hooks and feature-exclusive instrumentation.
- `raw ABI`: C callbacks, null/error conventions, exported symbols, and panic
  boundaries.

## Policy

- Prefer local unsafe blocks with nearby safety reasoning over broad
  `unsafe_op_in_unsafe_fn` allowances.
- Do not add module-level `allow(unsafe_op_in_unsafe_fn)`.
- Any new unsafe wrapper must document caller obligations.
- Any new safe wrapper around raw allocator state must explain how it enforces
  ownership, lifetime, and aliasing invariants.

