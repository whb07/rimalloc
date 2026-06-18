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

Recent cleanup removed unused `core_types` re-exports, the redundant crate-level `clippy::nonminimal_bool` allowance, the internal helper, allocation override, heap, init, allocation, aligned allocation, free-path, page, bitmap, random, page-map, arena-meta, tracking, options, stats, OS boundary, libc helper, POSIX allocation, atomic/lock, primitive TLS/helper, thread-heap, Emscripten primitive, WASI primitive, macOS primitive, and Unix primitive `clippy::missing_safety_doc` module allowances after documenting their unsafe boundaries, and a redundant thread-local safety-doc allowance. The remaining broad allowances are mostly C-port compatibility (`non_camel_case_types`, C ABI globals, and `unsafe_op_in_unsafe_fn`) or intentional type scaffolding that is not yet live at every future call site. Remaining localized Clippy allowances are limited to secure/debug feature constant comparisons and C-compatible arena/OS helper arity.

The typed atomic layer now centralizes thread-free word load, CAS, ownership-claim, and ownership-release operations through `AtomicThreadFree(AtomicUsizeField)`, keeping pointer-to-atomic casts behind the field wrapper required by `TYPES_SPEC.md` while preserving the existing atomic operation shapes. Page queue assertions no longer require the page module's broad `clippy::nonminimal_bool` allowance, bitmap condition/range/multiple-of/saturating-sub helpers no longer require `clippy::collapsible_if`, `clippy::manual_range_contains`, `clippy::manual_is_multiple_of`, or `clippy::implicit_saturating_sub`, bit/random/bitmap helpers no longer require `clippy::missing_const_for_fn`, and TLD/thread-heap sentinels no longer require `clippy::manual_dangling_ptr`.

Benchmark verification after the `AtomicThreadFree(AtomicUsizeField)` cleanup refreshed the release cdylib and ran the full `mimalloc-bench` `allt` suite at `-n=3`: Rust geomean was `0.996245` (`-0.38%` vs C), matching the `BENCHMARK_RESULTS.md` parity envelope (`0.995237`, `-0.48%`). Focused `-n=5` repeats cleared the noisy cache hot path (`cache-scratchN` median tie, mean Rust-favored) and kept glibc/shbench hot-path cases within the established noise range; redis and larson-sized remained small noisy watch items without a stable Rust absolute-time regression against the baseline.

`tests/release_fast_path_audit.rs` now includes a cross-cutting data-type acceptance audit for the `TYPES_SPEC.md` Section 18 invariants that are enforceable from source shape: `core_types` module ownership, memid union access limited to `MemId::view`/constructors, pointer-to-atomic casts limited to typed atomic field wrappers, no `Deref`/`DerefMut` on core metadata wrappers, block-link raw field access limited to the free-list wrapper layer, and typed free/reclaim/page-queue signatures.

Padding metadata access now routes through `core_types::PaddingPtr`, `Padding`, `PaddingCanary`, and `PaddingDelta` in the allocation/free support paths. `PaddingCanary` is constructed from `PageKeys`, and `PaddingDelta` is constructed with the usable-size bound required by `TYPES_SPEC.md` Section 14. The source audit guards that raw `mi_padding_t` field access remains confined to the raw type definition and padding wrapper, moving the Section 14 padding scaffold closer to the centralization rule without changing the default release allocator path where `MI_PADDING == 0`.

A Phase 6 dead-code probe confirmed the remaining `core_types` dead-code surface covers required staged typestate/scalar scaffolding from `TYPES_SPEC.md`, not accidental live-code silence. The former broad `core_types` `#![allow(dead_code)]` is now narrowed to module-scoped `#[expect(dead_code)]` markers on the compiler-proven scaffold modules, while live modules such as free actions, validation extras, and padding remain unmarked. The unused pointer-wrapper `as_non_null` escape hatch was removed so pointer wrappers expose only the constructor/raw-pointer surface required by the spec.

Follow-up Phase 6 dead-code cleanup removed stale module-wide `dead_code` allowances from the public header/static facades, stats/tracking/POSIX allocation boundaries, bitmap/libc/override/primitive boundary modules, the active Unix primitive layer, arena metadata, thread-heap helpers, and the allocation/free hot-path modules. Remaining broad `dead_code` allowances are now limited to `internal` re-export compatibility, non-Linux primitive targets that are not compiled on this platform, and a small set of arena C-compatibility definitions/helpers; `core_types` uses scoped expectations instead of a broad allowance. The active hot-path modules now rely on item-level allowances only where a specific staged compatibility helper remains.

Additional item-level cleanup removed stale `dead_code` markers from heap thread-local helpers, page/OS/libc helper paths, option/message initialization, process/TLS initialization, and the arena visit, metadata, allocation, abandoned-claim, and purge helpers. `arena.rs` no longer carries `dead_code` allowances; its remaining localized allowances are C naming compatibility and C-shaped helper arity. The remaining active-platform `dead_code` allowances are limited to specific staged compatibility or fallback items in allocation, bitmap, libc, OS huge-page cleanup, options, init/TLS/cpuid, stats, and Unix clock fallback code, plus the already-audited `core_types` strict-type scaffold and `internal` re-export compatibility.

`tests/release_fast_path_audit.rs` now enforces that remaining `dead_code` allowances stay on that explicit compatibility/scaffold allowlist, and asserts that `arena.rs`, `free.rs`, and `page.rs` do not regain `dead_code` suppressions.

Stale localized Clippy allowances were removed from the non-Linux Unix clock fallback and the `mi_manage_memory` C API wrapper. The source audit now also enforces the remaining localized Clippy allowlist by file/lint count.

The tracking shim, `core_types`, POSIX allocation shims, arena metadata, libc helpers, primitive TLS helpers, random helpers, and thread-local helpers no longer need broad `unsafe_op_in_unsafe_fn` module allowances; strict-type unchecked constructors, raw padding access, errno writes, realloc pointer updates, environment-duplication out-parameters, meta-page pointer arithmetic/bitmap access, C-string reads/writes, primitive TLS key/heap-slot access, random context dereferences, weak-context reinitialization calls, and TLS claim callback writes now use explicit unsafe blocks. The remaining broad unsafe-op allowances are enforced by `tests/release_fast_path_audit.rs` as an explicit mechanical raw-pointer module allowlist, with those cleaned modules guarded against regaining that suppression.

Stale C naming allowances were removed from the static and tracking facades. The source audit now enforces the remaining C naming, macro, and ctypes allowlist by file/lint count, with `static.rs` and `track.rs` guarded against regaining those suppressions.

No benchmark rerun was needed for the allowance and padding-wrapper narrowing because the lint/audit changes do not affect generated allocator logic, and default release builds compile the padding metadata branch out (`MI_PADDING == 0`). The latest runtime benchmark evidence remains the post-`AtomicThreadFree(AtomicUsizeField)` verification above against `BENCHMARK_RESULTS.md`.

**Action Items:**
1. **Remove Unused Types:** In progress. Do not delete C ABI aliases still required by `raw`/compat/public module boundaries.
2. **Resolve Clippy Warnings:** In progress. `cargo clippy --all-targets -- -D warnings` is clean with the remaining intentional allowances.
3. **Safety Documentation:** Done for the former broad `clippy::missing_safety_doc` allowances. Continue adding local `/// # Safety` docs whenever new public unsafe C-shaped boundaries are introduced or exposed.
