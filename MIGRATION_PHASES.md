# Remaining mimalloc refactoring phases

> **Note on Traceability:**
> This document summarizes the migration order for integrating the strict types. 
> Detailed type definitions, constructors, invariants, and the `DT-1`..`DT-10` PR sequence 
> are defined in `TYPES_SPEC.md` (specifically Section 19). Each phase below corresponds 
> directly to one or more `DT-*` items from that specification. `TYPES_SPEC.md` remains 
> the authoritative source for acceptance criteria and invariants.

## Phase 3: Validation Boundaries (Covers DT-6)

**Goal:** Establish a strict trusted vs. untrusted pointer boundary before any free action can take place.

**Current State:** Functions like `mi_free_block_local` and `mi_free_block_mt` implicitly trust the pointer they are given, relying on `mi_check_is_double_free` and manual interior pointer checks inside the free path itself.

**Action Items:**
1. **Refactor `mi_validate_ptr_page` and `mi_validate_block_from_ptr`:**
   - Modify these to return `PointerLookup::Valid` or `PointerLookup::Invalid`.
   - The validation process must handle `BlockOffset` and determine if the pointer is at `PointerLocation::BlockStart`, `PointerLocation::Interior`, or `PointerLocation::Guarded`.
2. **Implement `ValidatedFree` Construction:**
   - In `mi_free_ex` (and any other free entry points), force the lookup result into a `ValidatedFree` object before proceeding.
   - If `PointerLookup::Invalid`, route to the appropriate error handler (e.g., ignoring or panicking based on `PointerInvalidReason`).
3. **Guard Blocks:**
   - Extract the `was_guarded` boolean into the explicit `WasGuarded` enum when constructing the `BlockForFree`.

## Phase 4: Typestates & The Core State Machine (Covers DT-8, DT-9, DT-10)

**Goal:** Prevent illegal state transitions in the allocator's core state machine (the queues and free lists). This is the most complex phase.

**Current State:** The allocator freely passes `*mut mi_page_t` everywhere. Functions like `_mi_page_retire` or `mi_page_unfull` rely on raw assertions to check if a page is actually in the correct queue or has the correct flags.

**Action Items:**
1. **Migrate Free Function Signatures (DT-9):**
   - Update `mi_free_block_local` to accept `(page: LocalPage, block: BlockForFree, mode: LocalFreeMode)`.
   - Update `mi_free_block_mt` to accept `(page: ForeignPage, block: BlockForFree)`.
   - Update the reclaim loop (`_mi_theap_page_reclaim`) to strictly accept `OwnedAbandonedPage`.
2. **Implement Typed Queues (DT-10):**
   - Refactor `mi_page_queue_enqueue_from` and `_mi_page_unfull` to use the strict state transitions (`move_to_full_queue`, `unfull_page`).
   - Replace raw `*mut mi_page_queue_t` arguments with `NormalPageQueue`, `FullPageQueue`, etc.
3. **Enforce Free-List Pushing/Popping (DT-8):**
   - Update the core allocation loop (`mi_malloc_generic`) to extract blocks by calling `pop()` on an `AvailableFreeList`.
   - Update `mi_free_block_local` to return blocks by calling `push()` on `LocalFreeList`.

## Phase 5: Scalar and Layout Enforcement (Covers DT-1, DT-2, DT-3, DT-11)

**Goal:** Eliminate all instances of bare `usize` used for domain-specific metrics in the internal API, ensuring mathematical operations (like padding addition or alignment) cannot silently overflow.

**Current State:** Functions accept bare `usize` for size, alignment, bin indices, and slice counts.

**Action Items:**
1. **Update Allocation Shims:**
   - Refactor internal `malloc`/`calloc`/`realloc` entry points to immediately construct a `LayoutRequest`, `CallocRequest`, or `ReallocRequest`.
   - Propagate these request objects deep into the allocator (e.g., pass `LayoutRequest` down to `_mi_page_malloc` instead of raw `size`).
2. **Update Math Helpers:**
   - Migrate `mi_good_size`, `mi_usable_size`, and padding calculations to accept and return `UsableSize`, `BlockSize`, and `PaddedSize`.
   - Migrate `mi_bin` to return the typed `Bin` and `DirectBin` wrappers.
3. **Arena Slices:**
   - Replace raw slice calculations with the `SliceRange` object we scaffolded in Phase 1.

## Phase 6: Code Cleanup & Linting

**Goal:** Ensure the final Rust implementation is idiomatic, clean, and free of the `allow` attributes used during the structural port.

**Action Items:**
1. **Remove Unused Types:** Delete any C-style aliases (`mi_memid_t`, `mi_page_t`, etc.) from `src/types.rs` that have been successfully obsoleted by the new `core_types`.
2. **Resolve Clippy Warnings:** Address all remaining lints (e.g., `clippy::if_same_then_else`, `clippy::missing_safety_doc`).
3. **Safety Documentation:** Ensure every `unsafe fn` or `unsafe {}` block has a corresponding `/// # Safety` or `// SAFETY:` comment justifying its correctness against the new strict invariants.