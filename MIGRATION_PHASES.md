# Strict-Type Migration Phases

This file records the current implementation status for the strict-type
migration described by `TYPES_SPEC.md` and tracked in `TODO.md`.

The migration is intentionally staged. Raw C ABI shims remain C-shaped while
internal allocator code is moved toward crate-private wrapper domains.

## Phase Status

| Phase | Status | Notes |
|---|---|---|
| Phase 0: Status, Inventory, And Enforcement | In progress | `docs/strict-types.md` points here for status. `scripts/check-strict-types.py` and `strict-type-allowlist.txt` define the first checked raw-pointer inventory. |
| Phase 1: Core Module Privacy And Witness Construction | In progress | Pointer wrappers are private-field transparent newtypes. Several witness constructors remain staged or directly constructible inside `core_types`. |
| Phase 2: Scalar Domain Completion | In progress | Core size/bin/alignment wrappers exist; several domain wrappers and migrations remain outstanding. |
| Phase 3: Pointer Domain Completion | In progress | Non-null pointer wrappers exist; broad field-helper migration remains outstanding. |
| Phase 4-19 | Not complete | Later phases remain tracked in `TODO.md`. |

## Enforcement Status

Current checked rules:

- internal raw pointer parameters for `mi_page_t`, `mi_block_t`, `mi_heap_t`,
  `mi_theap_t`, `mi_arena_t`, and `mi_subproc_t` must either be in a raw
  boundary file or be listed in `strict-type-allowlist.txt`;
- direct memid union reads stay behind `src/core_types/memid.rs` or raw layout
  modules;
- direct `mi_block_s.next` access stays behind
  `src/core_types/freelist_wrappers.rs`;
- direct `xthread_id` and `xthread_free` bit operations stay behind the typed
  page/free-list wrapper modules;
- pointer-to-atomic casts stay behind `src/core_types/atomic_wrappers.rs`;
- broad metadata references, arbitrary pointer-to-`usize` round trips, and
  heap-allocating Rust library types are source-checked with documented
  allowlist entries where the current translation still requires them.

The allowlist is debt, not design permission. Each entry must name a removal
task or explain why it is a permanent C/compat boundary.
