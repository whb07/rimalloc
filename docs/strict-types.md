# Strict Type Architecture

`TYPES_SPEC.md` is the acceptance source for the allocator's internal type
model. `MIGRATION_PHASES.md` records the implementation status. This document
is the short maintainer map for the current `src/core_types` modules.

The strict-type layer is crate-private. It exists to make allocator invariants
explicit without changing the raw C ABI or hot-path code shape. Raw C-compatible
types remain in `src/types.rs` and C-shaped declarations remain behind
`mechanical_mimalloc::raw`.

## Status Labels

- `live`: used by current allocator paths.
- `live + staged`: used by current paths and also contains future-facing
  wrappers required by `TYPES_SPEC.md`.
- `staged`: retained as checked design scaffold for a required invariant, but
  not yet broadly consumed by live allocator paths.
- `compat boundary`: exists to bridge typed internals with translated C-shaped
  code.

## Module Map

| Module | Status | Invariant |
|---|---|---|
| `scalar` | live + staged | Separates requested, effective, padded, usable, block, bin, slice, and alignment domains. |
| `ptr` | live + staged | Provides non-null transparent metadata/data pointer wrappers without `Deref` or `DerefMut`. |
| `layout` | live | Centralizes malloc, calloc, realloc, zeroing, padding, alignment, and huge-alignment request validation. |
| `page` | live | Splits tagged page owner bits, page flags, and local/remote fast-path classification. |
| `freelist` | live | Packs and unpacks thread-free list owner/head words. |
| `freelist_wrappers` | live | Owns encoded block links, guarded tags, and available/local/remote free-list movement. |
| `validation` | live + staged | Models pointer lookup results, guarded/interior locations, and validated free requests. |
| `validation_extra` | live | Carries allocated/free block witnesses used by free-list and allocation paths. |
| `page_states` | live + staged | Provides page typestate witnesses for local, foreign, full, abandoned, and all-free paths. |
| `page_queues` | live | Requires typed queue/page witnesses for full/unfull transitions. |
| `memid` | live + staged | Restricts union-arm interpretation to typed `MemId::view` and constructors. |
| `atomic_wrappers` | live | Centralizes pointer-to-atomic field casts and fixed acquire/release/acqrel metadata operations. |
| `padding` | live | Owns padding pointer, canary, and delta access for allocation/free support paths. |
| `arena` | live + staged | Models arena slice ranges and bounds-checked slice counts. |
| `heap` | staged | Documents heap/thread-heap/subprocess witness shapes before broader adoption. |
| `free_actions` | live | Carries local free mode options as an explicit value. |

## Dead-Code Expectations

`core_types` intentionally uses scoped `#[expect(dead_code)]` markers on modules
that still contain staged acceptance-surface types. These are not a license to
add unused code. A new wrapper should be marked staged only when it corresponds
to a concrete `TYPES_SPEC.md` invariant and has a planned adoption path.

Do not add a broad module-level `#![allow(dead_code)]` to `core_types`.

## Verification

The release fast-path audit checks the current source-shape contract:

- `core_types` owns the semantic wrapper modules,
- metadata wrappers do not implement `Deref` or `DerefMut`,
- memid union reads stay behind `MemId::view` or typed constructors,
- pointer-to-atomic casts stay behind `Atomic*Field` wrappers,
- block `next` access stays behind free-list wrappers,
- padding metadata access stays behind `PaddingPtr`,
- typed free/reclaim/page-queue signatures do not regress to raw page states.

Run:

```sh
cargo test --quiet --test release_fast_path_audit
cargo test --quiet --lib
```

Use `scripts/check-layout.sh` when ABI/layout changes touch the strict-type
boundary.
