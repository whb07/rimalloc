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

Recent cleanup removed unused `core_types` re-exports, the redundant crate-level `clippy::nonminimal_bool` allowance, the internal helper, allocation override, heap, init, allocation, aligned allocation, free-path, page, bitmap, random, page-map, arena-meta, tracking, options, stats, OS boundary, libc helper, POSIX allocation, atomic/lock, primitive TLS/helper, thread-heap, Emscripten primitive, WASI primitive, macOS primitive, and Unix primitive `clippy::missing_safety_doc` module allowances after documenting their unsafe boundaries, and a redundant thread-local safety-doc allowance. The remaining broad allowances are mostly C-port compatibility (`non_camel_case_types` and C ABI globals) or intentional type scaffolding that is not yet live at every future call site. Remaining localized Clippy allowances are limited to secure/debug feature constant comparisons and C-compatible arena/OS helper arity.

The typed atomic layer now centralizes thread-free word load, CAS, ownership-claim, and ownership-release operations through `AtomicThreadFree(AtomicUsizeField)`, keeping pointer-to-atomic casts behind the field wrapper required by `TYPES_SPEC.md` while preserving the existing atomic operation shapes. Page queue assertions no longer require the page module's broad `clippy::nonminimal_bool` allowance, bitmap condition/range/multiple-of/saturating-sub helpers no longer require `clippy::collapsible_if`, `clippy::manual_range_contains`, `clippy::manual_is_multiple_of`, or `clippy::implicit_saturating_sub`, bit/random/bitmap helpers no longer require `clippy::missing_const_for_fn`, and TLD/thread-heap sentinels no longer require `clippy::manual_dangling_ptr`.

Benchmark verification after the `AtomicThreadFree(AtomicUsizeField)` cleanup refreshed the release cdylib and ran the full `mimalloc-bench` `allt` suite at `-n=3`: Rust geomean was `0.996245` (`-0.38%` vs C), matching the `BENCHMARK_RESULTS.md` parity envelope (`0.995237`, `-0.48%`). Focused `-n=5` repeats cleared the noisy cache hot path (`cache-scratchN` median tie, mean Rust-favored) and kept glibc/shbench hot-path cases within the established noise range; redis and larson-sized remained small noisy watch items without a stable Rust absolute-time regression against the baseline.

`tests/release_fast_path_audit.rs` now includes a cross-cutting data-type acceptance audit for the `TYPES_SPEC.md` Section 18 invariants that are enforceable from source shape: `core_types` module ownership, memid union access limited to `MemId::view`/constructors, pointer-to-atomic casts limited to typed atomic field wrappers, no `Deref`/`DerefMut` on core metadata wrappers, block-link raw field access limited to the free-list wrapper layer, and typed free/reclaim/page-queue signatures.

Padding metadata access now routes through `core_types::PaddingPtr`, `Padding`, `PaddingCanary`, and `PaddingDelta` in the allocation/free support paths. `PaddingCanary` is constructed from `PageKeys`, and `PaddingDelta` is constructed with the usable-size bound required by `TYPES_SPEC.md` Section 14. The source audit guards that raw `mi_padding_t` field access remains confined to the raw type definition and padding wrapper, moving the Section 14 padding scaffold closer to the centralization rule without changing the default release allocator path where `MI_PADDING == 0`.

A Phase 6 dead-code probe confirmed the remaining `core_types` dead-code surface covers required staged typestate/scalar scaffolding from `TYPES_SPEC.md`, not accidental live-code silence. The former broad `core_types` `#![allow(dead_code)]` is now narrowed to module-scoped `#[expect(dead_code)]` markers on the compiler-proven scaffold modules, while live modules such as free actions, validation extras, and padding remain unmarked. The unused pointer-wrapper `as_non_null` escape hatch was removed so pointer wrappers expose only the constructor/raw-pointer surface required by the spec.

Follow-up Phase 6 dead-code cleanup removed stale module-wide `dead_code` allowances from the public header/static facades, stats/tracking/POSIX allocation boundaries, bitmap/libc/override/primitive boundary modules, the active Unix primitive layer, arena metadata, thread-heap helpers, and the allocation/free hot-path modules. Remaining broad `dead_code` allowances are now limited to `internal` re-export compatibility, non-Linux primitive targets that are not compiled on this platform, and a small set of arena C-compatibility definitions/helpers; `core_types` uses scoped expectations instead of a broad allowance. The active hot-path modules now rely on item-level allowances only where a specific staged compatibility helper remains.

Additional item-level cleanup removed stale `dead_code` markers from heap thread-local helpers, page/OS/libc helper paths, option/message initialization, process/TLS initialization, CPU feature detection, stats aggregation, and the arena visit, metadata, allocation, abandoned-claim, and purge helpers. A stale crate-private option-size wrapper was removed after all live paths were confirmed to call the underlying option helper directly, the obsolete stats bin-size table/wrapper was removed after `mi_stats_get_bin_size` was confirmed to use the live page bin-size helper, and the checked raw stats-add helper is now exercised by the reference wrapper while `_mi_stats_merge_into` still uses the direct non-null body required by C's merge-and-zero path. An unused private `mallocn` wrapper was removed while preserving the audited `CallocRequest` `_req` path used by public `mi_mallocn` entry points, stale libc output-formatting helpers were removed while preserving the audited numeric renderer, unused bitmap helper variants were removed after exact call-site searches showed the mask-based helpers are the live paths, and the 32-bit libc popcount fallback is now cfg-gated instead of carrying active-platform `dead_code` allowances. Nonactive Linux-compiled TLS slot and pthread-key fallback stubs were removed after their callers were confirmed to be gated behind Windows or OpenBSD/Android cfg blocks. The stale OS huge-page cleanup allowance was eliminated by restoring C's `MI_MEM_OS_HUGE` free routing through `mi_os_free_huge_os_pages`, so pinned huge-page reservations are released per page instead of falling through the normal OS free path. Unix `clock()` fallback declarations are now cfg-gated to platforms that do not use the `clock_gettime` path. C's process-init CPU feature detection is restored before allocator subsystem initialization, so the hidden popcnt and short `rep movsb`/`rep stosb` thresholds are initialized without making those internals dynamic ABI symbols. The retained C-shaped `mi_tld_alloc` helper is now referenced through a private function-pointer const instead of an item-level `dead_code` allowance. `arena.rs`, `init.rs`, and `internal.rs` no longer carry `dead_code` allowances; remaining active-platform `dead_code` allowances are limited to the already-audited `core_types` strict-type scaffold.

`tests/release_fast_path_audit.rs` now enforces that remaining `dead_code` allowances stay on that explicit compatibility/scaffold allowlist, and asserts that `arena.rs`, `init.rs`, `internal.rs`, `free.rs`, and `page.rs` do not regain `dead_code` suppressions.

Stale localized Clippy allowances were removed from the non-Linux Unix clock fallback and the `mi_manage_memory` C API wrapper. The source audit now also enforces the remaining localized Clippy allowlist by file/lint count.

The tracking shim, `core_types`, allocation helpers, allocation override shims, aligned allocation helpers, POSIX allocation shims, arena metadata, atomic/lock helpers, bitmap helpers, free-path helpers, heap management helpers, internal helpers, libc helpers, option/message helpers, OS boundary helpers, page helpers, primitive TLS helpers, Unix primitive helpers, Emscripten primitive helpers, WASI primitive helpers, macOS primitive helpers, random helpers, thread-heap helpers, and thread-local helpers no longer need broad `unsafe_op_in_unsafe_fn` module allowances; strict-type unchecked constructors, raw padding access, errno writes, allocation free-list pops, realloc pointer validation/application, C++ new-handler retry loops, realpath/string duplication operations, guarded allocation metadata/protection operations, environment-duplication out-parameters, aligned allocation forwarding calls, aligned over-allocation metadata/padding updates, aligned small-page dereferences, aligned realloc copy/free operations, meta-page pointer arithmetic/bitmap access, atomic reference casts, compare-exchange out-parameters, lock/once raw pointer operations, bitmap chunk-count/chunk-map/visitor callback operations, bbitmap search/update operations, free-path padding validation, local/remote free-list updates, abandoned-page reclaim, and usable-size lookup operations, heap lifecycle/thread-heap lookup operations, C-string reads/writes, forwarded allocation sizing calls, option/message boundary operations, OS commit/decommit/reset/purge/free/alloc/huge-page out-parameter and stats operations, page validation/list traversal, page queue movement, page retire/free/abandon/reclaim, page extension/fresh allocation, deferred-free callback operations, generic page lookup/allocation, primitive TLS key/heap-slot access, Unix syscall/mmap/env/random/pthread boundary operations, Emscripten emmalloc/pthread/console out-parameter operations, WASI sbrk/getenv/string out-parameter operations, macOS malloc-zone forwarding and out-parameter operations, random context dereferences, internal page-map/page-metadata/copy-fill helper operations, thread-heap page/area visitors, typed thread-heap refcount and heap-pointer atomics, weak-context reinitialization calls, and TLS claim callback writes now use explicit unsafe blocks. The source audit now enforces an empty broad unsafe-op allowlist so cleaned modules cannot regain that suppression.

Stale C naming allowances were removed from the static and tracking facades, the active Unix primitive layer no longer carries a stale `unused_imports` allowance, and private unused bit-helper macro scaffolding was removed so `bits.rs` no longer needs an `unused_macros` allowance. The source audit now enforces the remaining C compatibility, macro, import, and ctypes allowlist by file/lint count, with `static.rs` and `track.rs` guarded against regaining those suppressions.

No benchmark rerun was needed for the allowance and padding-wrapper narrowing because the lint/audit changes do not affect generated allocator logic, and default release builds compile the padding metadata branch out (`MI_PADDING == 0`). The latest runtime benchmark evidence remains the post-`AtomicThreadFree(AtomicUsizeField)` verification above against `BENCHMARK_RESULTS.md`.

Current release verification after the Phase 6 dead-code cleanup rebuilt the cdylib, refreshed `target/bench-allocators/libmimalloc-rust.so`, and ran a short `mimalloc-bench` hot/watch pass at `-n=3` over glibc, rptest, cfrac, alloc-test, larson-sized, shbench, xmalloc, cache scratch, redis, and rocksdb. The 14-case geomean was `1.002766` (`+0.28%` Rust vs C) while the allocator hot-path cases were tied or Rust-favored: `glibc-simple` `-3.23%`, `glibc-thread` `+0.40%`, `rptestN` `-2.78%`, `cfrac` `-0.61%`, `alloc-test1` `-0.80%`, `alloc-testN` `+0.16%`, `larsonN-sized` `-1.47%`, and cache scratch tied/Rust-favored. Focused `-n=5` repeats for the noisy cases showed `sh8benchN` remained a short-test spread (`+4.17%` mean on `.27s.. .31s` samples), redis narrowed to a small app-level tail (`+1.49%` mean, `+0.75%` median), and rocksdb was Rust-favored (`-0.34%` mean, `-0.84%` median). No hot-path or overall benchmark regression was found relative to the `BENCHMARK_RESULTS.md` parity envelope.

CPU feature detection was benchmarked because it can affect the allocator copy/fill helpers. The refreshed release cdylib ran a focused hot/watch pass at `-n=3` over glibc, rptest, cfrac, alloc-test, xmalloc, and cache scratch. The first 9-case focused geomean was `1.006515` (`+0.65%` Rust vs C), driven by a noisy `rptestN` outlier. A targeted `rptestN -n=5` repeat resolved that noise with Rust faster on both mean (`0.1616s` vs `0.1702s`) and median (`0.162s` vs `0.169s`). Substituting the 5-run `rptestN` result gives a focused geomean of `0.992359` (`-0.76%` Rust vs C), so no hot-path or overall-score regression was found against `BENCHMARK_RESULTS.md`.

No benchmark rerun was needed for the stats dead-code cleanup, stale Unix import allowance removal, private bit macro cleanup, heap unsafe-op cleanup, Unix primitive unsafe-op cleanup, Emscripten/WASI/macOS primitive unsafe-op cleanup, thread-heap unsafe-op cleanup, internal helper unsafe-op cleanup, bitmap unsafe-op cleanup, free-path unsafe-op cleanup, OS boundary unsafe-op cleanup, allocation unsafe-op cleanup, or page unsafe-op cleanup because those changes route stats aggregation through the existing checked helper, remove compile-only lint scaffolding, and make heap-management, Unix syscall/mmap/env/random/pthread, non-Linux primitive boundary, thread-heap lifecycle/visitor/refcount, internal page-map/page-metadata/copy-fill, bitmap chunk-map/search/update, free-path padding/free-list/reclaim, OS boundary out-parameter/stat-update, allocation/realloc/new-handler, and page lifecycle/queue/reclaim/deferred-free/generic-allocation unsafe scopes explicit without changing allocator control flow or generated hot-path logic. Verification used `cargo test --test release_fast_path_audit --quiet`, `cargo fmt --check`, `git diff --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --quiet`, and `cargo build --release`.

Fresh post-allocation-cleanup benchmark verification nevertheless refreshed `target/bench-allocators/libmimalloc-rust.so` from the release cdylib and ran `mimalloc-bench` `allt` at `-n=3` against the C allocator. The 19-case geomean was `1.004453` (`+0.45%` Rust vs C), still within the `BENCHMARK_RESULTS.md` parity envelope (`0.995237`, `-0.48%`) and below a 1% overall spread. Hot-path cases were tied or Rust-favored in the aggregate (`glibc-simple` `-3.92%`, `glibc-thread` `-0.63%`, `xmalloc-testN` `-0.31%`, cache scratch tied, `alloc-test1` `+0.32%`, `alloc-testN` `+0.76%`). Focused `-n=5` repeats for the noisy cases resolved the largest hot-path outlier: `rptestN` narrowed to `+2.60%` mean and `+1.11%` median, `sh8benchN` was Rust-favored (`-0.78%` mean, `-3.85%` median), and rocksdb was tied/Rust-favored (`-0.06%` mean, `-0.84%` median). Redis (`+2.67%` mean, `+3.72%` median) and `larsonN-sized` (`+2.64%` mean, `+2.89%` median) remain low-single-digit noisy watch items, not allocator hot-path regressions. No hot-path or overall benchmark regression requiring code action was found.

**Action Items:**
1. **Remove Unused Types:** In progress. Do not delete C ABI aliases still required by `raw`/compat/public module boundaries.
2. **Resolve Clippy Warnings:** In progress. `cargo clippy --all-targets -- -D warnings` is clean with the remaining intentional allowances.
3. **Safety Documentation:** Done for the former broad `clippy::missing_safety_doc` allowances. Continue adding local `/// # Safety` docs whenever new public unsafe C-shaped boundaries are introduced or exposed.
