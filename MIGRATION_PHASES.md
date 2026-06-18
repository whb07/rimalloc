# mimalloc strict-type migration status

> **Note on Traceability:**
> This document summarizes the migration order for integrating the strict types. 
> Detailed type definitions, constructors, invariants, and the `DT-1`..`DT-10` PR sequence 
> are defined in `TYPES_SPEC.md` (specifically Section 19). Each phase below corresponds 
> directly to one or more `DT-*` items from that specification. `TYPES_SPEC.md` remains 
> the authoritative source for acceptance criteria and invariants.

## Phase 3: Validation Boundaries (Covers DT-6)

**Goal:** Establish a strict trusted vs. untrusted pointer boundary before any free action can take place.

**Status:** Implemented and guarded by `tests/release_fast_path_audit.rs`.

`mi_validate_ptr_page` and `mi_validate_block_from_ptr` now return `PointerLookup`, free entry points build `ValidatedFree` before dispatching to local or remote free, and guarded blocks carry `WasGuarded` through `BlockForFree`.

**Action Items:**
1. **Refactor `mi_validate_ptr_page` and `mi_validate_block_from_ptr`:** Done.
   - Modify these to return `PointerLookup::Valid` or `PointerLookup::Invalid`.
   - The validation process must handle `BlockOffset` and determine if the pointer is at `PointerLocation::BlockStart`, `PointerLocation::Interior`, or `PointerLocation::Guarded`.
2. **Implement `ValidatedFree` Construction:** Done.
   - In `mi_free_ex` (and any other free entry points), force the lookup result into a `ValidatedFree` object before proceeding.
   - If `PointerLookup::Invalid`, route to the appropriate error handler (e.g., ignoring or panicking based on `PointerInvalidReason`).
3. **Guard Blocks:** Done.
   - Extract the `was_guarded` boolean into the explicit `WasGuarded` enum when constructing the `BlockForFree`.

## Phase 4: Typestates & The Core State Machine (Covers DT-8, DT-9, DT-10)

**Goal:** Prevent illegal state transitions in the allocator's core state machine (the queues and free lists). This is the most complex phase.

**Status:** Implemented for the free/reclaim/queue/free-list paths targeted by DT-8, DT-9, and DT-10, with audit coverage.

`mi_free_block_local`, `mi_free_block_mt`, and `_mi_theap_page_reclaim` accept typed page-state wrappers. Queue movement goes through typed queue/page transitions while retaining raw compatibility shims at the C-shaped boundary. Allocation/free-list hot paths pop and push through `AvailableFreeList`, `LocalFreeList`, `EncodedPtr`, and `BlockLink`.

**Action Items:**
1. **Migrate Free Function Signatures (DT-9):** Done.
   - Update `mi_free_block_local` to accept `(page: LocalPage, block: BlockForFree, mode: LocalFreeMode)`.
   - Update `mi_free_block_mt` to accept `(page: ForeignPage, block: BlockForFree)`.
   - Update the reclaim loop (`_mi_theap_page_reclaim`) to strictly accept `OwnedAbandonedPage`.
2. **Implement Typed Queues (DT-10):** Done.
   - Refactor `mi_page_queue_enqueue_from` and `_mi_page_unfull` to use the strict state transitions (`move_to_full_queue`, `unfull_page`).
   - Replace raw `*mut mi_page_queue_t` arguments with `NormalPageQueue`, `FullPageQueue`, etc.
3. **Enforce Free-List Pushing/Popping (DT-8):** Done.
   - Update the core allocation loop (`mi_malloc_generic`) to extract blocks by calling `pop()` on an `AvailableFreeList`.
   - Update `mi_free_block_local` to return blocks by calling `push()` on `LocalFreeList`.

## Phase 5: Scalar and Layout Enforcement (Covers DT-1, DT-2, DT-3, DT-11)

**Goal:** Eliminate all instances of bare `usize` used for domain-specific metrics in the internal API, ensuring mathematical operations (like padding addition or alignment) cannot silently overflow.

**Status:** Implemented for the allocation/product-size, bin, usable-size, good-size, direct-bin, and arena-slice paths called out below, with audit coverage.

Raw `usize` remains at FFI, ABI, and compatibility boundaries, but the internal paths targeted here route through `LayoutRequest`, `CallocRequest`, `ReallocRequest`, `PaddedSize`, `UsableSize`, `BlockSize`, `Bin`, `DirectBin`, and `SliceRange`.

**Action Items:**
1. **Update Allocation Shims:** Done.
   - Refactor internal `malloc`/`calloc`/`realloc` entry points to immediately construct a `LayoutRequest`, `CallocRequest`, or `ReallocRequest`.
   - Propagate these request objects deep into the allocator (e.g., pass `LayoutRequest` down to `_mi_page_malloc` instead of raw `size`).
2. **Update Math Helpers:** Done for the audited allocator size/bin helper paths.
   - Migrate `mi_good_size`, `mi_usable_size`, and padding calculations to accept and return `UsableSize`, `BlockSize`, and `PaddedSize`.
   - Migrate `mi_bin` to return the typed `Bin` and `DirectBin` wrappers.
3. **Arena Slices:** Done.
   - Replace raw slice calculations with the `SliceRange` object we scaffolded in Phase 1.

## Phase 6: Code Cleanup & Linting

**Goal:** Ensure the final Rust implementation is idiomatic, clean, and free of the `allow` attributes used during the structural port.

**Status:** In progress.

Recent cleanup removed unused `core_types` re-exports, the redundant crate-level `clippy::nonminimal_bool` allowance, the heap, random, page-map, arena-meta, tracking, options, stats, OS boundary, libc helper, POSIX allocation, atomic/lock, primitive TLS/helper, thread-heap, Emscripten primitive, WASI primitive, macOS primitive, and Unix primitive `clippy::missing_safety_doc` module allowances after documenting their unsafe boundaries, and a redundant thread-local safety-doc allowance. The remaining broad allowances are mostly C-port compatibility (`non_camel_case_types`, C ABI globals, `unsafe_op_in_unsafe_fn`, and missing safety docs on still C-shaped unsafe boundaries) or intentional type scaffolding that is not yet live at every future call site.

**Action Items:**
1. **Remove Unused Types:** In progress. Do not delete C ABI aliases still required by `raw`/compat/public module boundaries.
2. **Resolve Clippy Warnings:** In progress. `cargo clippy --all-targets -- -D warnings` is clean with the remaining intentional allowances.
3. **Safety Documentation:** In progress. Continue converting broad `clippy::missing_safety_doc` allowances into local `/// # Safety` docs as C-shaped unsafe boundaries are made idiomatic.
