# Strict-Type Migration Phases

This file records the current implementation status for the strict-type
migration described by `TYPES_SPEC.md`.

The migration is intentionally staged. Raw C ABI shims remain C-shaped while
internal allocator code is moved toward crate-private wrapper domains.

## Phase Status

| Phase | Status | Notes |
|---|---|---|
| Phase 0: Status And Enforcement | Complete | `docs/strict-types.md` defines the current rules. `scripts/check-strict-types.py` checks the narrow boundaries Rust visibility cannot express. |
| Phase 1: Core Module Privacy And Witness Construction | Complete | Pointer and witness wrappers are private-field transparent newtypes with checked or explicitly unsafe constructors. |
| Phase 2: Scalar Domain Completion | Complete | Size, bin, alignment, count, index, page-map, errno, and memory-accounting domains are represented by crate-private wrappers where they cross internal APIs. |
| Phase 3: Pointer Domain Completion | Complete | Non-null pointer wrappers own metadata field helpers and avoid broad aliasing references. |
| Phase 4-19 | Complete | The strict-type migration is complete; raw C ABI and translated compatibility seams remain C-shaped by design. |
| Stage 20: Evidence Repair | Complete | Timed scratch allocation, facade labeling, persistent ownership topologies, resource evidence, provenance, and the reviewed schema-2 baseline are implemented. |
| Stage 21: Linear Capabilities | Complete | Validation, page-state, remote-list, and abandoned-page transitions are non-duplicable and compiler-checked in `src/types/linearity_tests.rs`. |
| Stage 22: Compatibility Policy | Complete | The concrete [`CompatPolicy` module](src/allocator/policy/) owns the compatibility decisions and reference tests. |
| Stage 23: Page-Batch Foundation | Complete | [`page_batch.rs`](src/types/page_batch.rs) implements bounded local, remote, and leased batches; performance evidence is recorded in [BENCHMARK_RESULTS.md](BENCHMARK_RESULTS.md). |

## Enforcement Status

Current checked rules:

- direct memid union reads stay behind `src/types/memid.rs` (internally aliased
  as `core_types::memid`) or raw layout
  modules;
- direct `mi_block_s.next` access stays behind
  `src/types/freelist_wrappers.rs`;
- direct `xthread_id` and `xthread_free` bit operations stay behind the typed
  page/free-list wrapper modules;
- pointer-to-atomic casts stay behind the atomic owner modules;
- raw page-state bridge calls stay at reviewed allocator ingress points;
- metadata pointer wrappers do not implement `Deref` or `DerefMut`;
- allocator internals do not introduce dynamic dispatch;
- linear capabilities are compiler-checked as neither `Copy` nor `Clone`;
- `src/types` modules reject crate-level `#![allow(dead_code)]`; retained
  proof/helper APIs must be wired through real call sites or narrow compile-time
  reachability references instead of broad lint suppression.
