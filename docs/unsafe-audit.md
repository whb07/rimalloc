# Unsafe Code Policy

Unsafe code is expected in this allocator, but each operation must have a local,
reviewable proof. This document defines the policy; it is not an inventory of
unsafe sites.

## Boundary rules

- Public C ABI and raw Rust entry points preserve mimalloc's pointer,
  nullability, ownership, alignment, callback, and threading contracts.
- The safe facade converts those contracts into owned values, lifetimes,
  initialized views, `Option`, and typed errors.
- Allocator metadata pointers originate from registered pages, arenas, heaps,
  thread heaps, subprocess state, or checked OS mappings before dereference.
- Pointer arithmetic stays within the allocation, page, arena, metadata, or
  buffer extent established by the surrounding operation.
- Cross-thread state changes use the typed atomic, ownership, abandonment,
  reclaim, callback, or lock protocol that owns the state.
- Unwinding must not cross an exported C ABI boundary.

The mechanical allocator core may retain C-shaped raw operations when that
keeps upstream behavior and generated code reviewable. New Rust-native helpers
should narrow the unsafe surface by validating once and returning a typed
value, not by wrapping and immediately reconstructing raw state.

## Local safety comments

Every unsafe block or implementation needs a nearby `// SAFETY:` comment that
states why the operation is valid at that point. Explain the applicable
validity, bounds, provenance, initialization, aliasing, lifetime, ownership,
threading, or unwind invariant. Public unsafe functions also require a
`# Safety` section describing caller obligations.

Safety comments are not assigned global IDs and are not mirrored into a
central evidence ledger. Tests and tools should be linked from the owning
module or protocol documentation only when that connection is not otherwise
obvious.

## Evidence

Choose evidence according to the risk:

- unit and property tests for checked arithmetic, layouts, state transitions,
  and boundary values;
- compile-time assertions for ownership capabilities and representation;
- ABI, layout, symbol, and C behavioral-parity checks for compatibility
  boundaries;
- Loom for modeled atomic publication and ownership protocols;
- Miri and strict-provenance tests for supported pointer-heavy paths;
- ASan, TSan, and Valgrind stress checks for allocation and lifecycle paths;
- fuzz regressions for parsers, pointer classification, allocation sequences,
  and state machines;
- release assembly and balanced benchmarks only for hot-path codegen claims.

Lexical unsafe counts are optional review aids. A count change is not itself a
safety result: the meaningful measures are fewer trusted callers, smaller
unsafe scopes, stronger typed boundaries, or removal of unsafe operations.

## Review checklist

For an unsafe change, verify:

1. Safe Rust cannot express the operation directly.
2. The local comment states the exact proof rather than naming a subsystem.
3. Callers cannot bypass validation or forge ownership accidentally.
4. The operation is scoped no wider than the invariant requires.
5. Relevant boundary, concurrency, provenance, or compatibility tests cover
   the changed behavior.
6. Hot-path and ABI behavior remain unchanged unless the change explicitly
   intends and validates a difference.
