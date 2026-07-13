# Strict-Type Migration Phases

This file records the current implementation status for the strict-type
migration described by `TYPES_SPEC.md` and tracked in `TODO.md`.

The migration is intentionally staged. Raw C ABI shims remain C-shaped while
internal allocator code is moved toward crate-private wrapper domains.

## Phase Status

| Phase | Status | Notes |
|---|---|---|
| Phase 0: Status, Inventory, And Enforcement | Complete | `docs/strict-types.md` points here for status. `scripts/check-strict-types.py` validates raw-pointer, bare-domain `usize`, source-shape, and allowlist-disposition rules. |
| Phase 1: Core Module Privacy And Witness Construction | Complete | Pointer and witness wrappers are private-field transparent newtypes with checked or explicitly unsafe constructors. |
| Phase 2: Scalar Domain Completion | Complete | Size, bin, alignment, count, index, page-map, errno, and memory-accounting domains are represented by crate-private wrappers where they cross internal APIs. |
| Phase 3: Pointer Domain Completion | Complete | Non-null pointer wrappers own metadata field helpers and avoid broad aliasing references. |
| Phase 4-19 | Complete | The strict-type TODO checklist is complete; raw C ABI and translated compatibility seams remain documented in the checked allowlist. |
| Stage 20: Evidence Repair | Complete | Timed scratch allocation, facade labeling, persistent ownership topologies, resource evidence, provenance, and the reviewed schema-2 baseline are implemented. |
| Stage 21: Linear Capabilities | Planned | Make validation, page-state, remote-list, and abandoned-page transition capabilities non-duplicable. |
| Stage 22: Compatibility Policy | Planned | Extract allocation policy behind an exactly equivalent `CompatPolicy` implementation and reference model. |
| Stage 23: Page-Batch Foundation | Planned | Introduce bounded page batches and integrate local and detached-remote batch transitions. |

## Enforcement Status

Current checked rules:

- internal raw pointer parameters for `mi_page_t`, `mi_block_t`, `mi_heap_t`,
  `mi_theap_t`, `mi_arena_t`, and `mi_subproc_t` must either be in a raw
  boundary file or be listed in `strict-type-allowlist.txt`;
- direct memid union reads stay behind `src/types/memid.rs` (internally aliased
  as `core_types::memid`) or raw layout
  modules;
- direct `mi_block_s.next` access stays behind
  `src/types/freelist_wrappers.rs`;
- direct `xthread_id` and `xthread_free` bit operations stay behind the typed
  page/free-list wrapper modules;
- pointer-to-atomic casts stay behind `src/types/atomic_wrappers.rs`;
- broad metadata references, arbitrary pointer-to-`usize` round trips, and
  heap-allocating Rust library types are source-checked with documented
  allowlist entries where the current translation still requires them.
- bare-domain `usize` parameters with allocation-size, alignment, count, index,
  bin, capacity, or slice-count names are rejected unless they are raw ABI
  compatibility functions or checked allowlist entries;
- every allowlist entry must carry a disposition beginning with `permanent:` or
  `remove:` so compatibility boundaries and migration debt are distinguishable;
- `src/types` modules reject crate-level `#![allow(dead_code)]`; retained
  proof/helper APIs must be wired through real call sites or narrow compile-time
  reachability references instead of broad lint suppression.

The allowlist is debt, not design permission. Each entry must name a removal
task or explain why it is a permanent C/compat boundary.
