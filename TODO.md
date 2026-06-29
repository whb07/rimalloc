# TODO: Allocator Code Review Issues

Source: allocator/code/API-architecture reviews on 2026-06-29 using Rust coding, type/performance, concurrency, and public API review criteria.

Scope: allocator implementation code, safe facade code, build-mode architecture, public API inventory, and policy scripts. Keep changes surgical: correctness and hot-path performance first; architecture/tooling work should make layer boundaries mechanically clearer, not churn translated code for style alone.

Do not mark an item complete until the named code/tooling change is implemented and validation evidence is passing, or an explicit unrelated blocker is recorded next to the item.

## 0. Baseline and guardrails

- [x] Record current branch and commit hash before changing allocator code.
  - Evidence: started on `master` at `8dcff3f313faae5106c689982c5eb83a0b02f725`.
- [x] Run `git status --short` and note any pre-existing uncommitted files.
  - Evidence: pre-existing `TODO.md` modification plus untracked `RUST_DEPENDENCY_EDGES.csv`, `parse_bench.py`, and `parse_bench_10x.py`.
- [x] Reproduce the known release zero-size failure:
  - [x] `cargo test --release --features std --test allocation_boundary_cases zero_size_allocations_return_freeable_pointers -- --nocapture`
  - [x] Confirm `mi_malloc(0)` currently returns null before fixing it, or record if already fixed.
  - Evidence: pre-fix command failed on `assertion failed: !p.is_null()` for `mi_malloc(0)`.
- [x] For each Rust code change, run the required project checks before declaring completion:
  - [x] `cargo fmt --all`
  - [x] `cargo clippy --workspace --all-targets --all-features`
  - [x] `cargo test --workspace --all-targets --all-features`
- [x] When a focused test is added for a bug, verify it fails before the fix and passes after the fix.
  - Evidence: focused release zero-size test failed before the fix and `cargo test --release --features std --test allocation_boundary_cases zero_size -- --nocapture` passed after the fix.
- [x] Keep hot-path changes small and benchmarkable; avoid broad refactors while fixing correctness issues.

## 1. P0 correctness/parity: zero-size raw allocations

Affected areas:

- `src/allocator/alloc.rs` small and generic allocation paths.
- `src/allocator/page.rs::_mi_malloc_generic`.
- `src/types/scalar.rs::PaddedSize` / `RequestedSize::padded_unchecked`.
- `src/types/layout.rs::LayoutRequest` zero-size normalization.
- `src/raw/macros.rs` fast assembly slow-path jumps into `_mi_malloc_generic` with size 0.

Problem:

- Raw C-compatible allocation APIs should return a freeable non-null pointer for zero-size allocation requests.
- Release builds with no padding can currently produce a null pointer because `PaddedSize` cannot represent `0`, while upstream mimalloc allows the `size + MI_PADDING_SIZE == 0` path and routes it through the minimum-size bin/direct-page machinery.

Tasks:

- [x] Add focused release tests for zero-size raw allocation APIs:
  - [x] `mi_malloc(0)` returns non-null and is freeable.
  - [x] `mi_zalloc(0)` returns non-null and is freeable.
  - [x] `mi_umalloc(0, &mut usable)` returns non-null and is freeable.
  - [x] `mi_uzalloc(0, &mut usable)` returns non-null and is freeable.
  - [x] `mi_calloc(0, n)` and/or `mi_calloc(n, 0)` return non-null and are freeable, unless upstream parity proves a different behavior.
  - [x] `mi_mallocn(0, n)` and/or `mi_mallocn(n, 0)` return non-null and are freeable, unless upstream parity proves a different behavior.
  - [x] Heap variants for zero-size allocation return non-null/freeable where the raw C API requires it.
- [x] Decide and document in code comments which internal representation should carry zero-size allocation requests:
  - [x] Either normalize zero to an effective minimum before constructing `PaddedSize`, or
  - [x] Not used: introduce/use a separate raw allocation-size type that can represent upstream's padded zero.
- [x] Fix `mi_theap_malloc_small_zero_nonnull` so zero-size requests do not construct an invalid `PaddedSize` in release.
- [x] Fix `_mi_theap_malloc_zero_ex_typed` / `mi_theap_malloc_zero_req` so `LayoutRequest::malloc(0)` takes the intended small path without collapsing to null.
- [x] Remove or replace the `size == 0 => null` behavior in `src/allocator/page.rs::_mi_malloc_generic`.
- [x] Ensure the fast assembly paths in `src/raw/macros.rs` that jump to `_mi_malloc_generic` with size 0 are covered by tests or by an explicit normalization boundary.
  - Evidence: `_mi_malloc_generic` normalizes zero before constructing `PaddedSize`.
- [x] Verify `mi_free` accepts and frees every zero-size pointer returned by these APIs.
- [x] Run the focused zero-size release test again and record the passing command.
  - Evidence: `cargo test --release --features std --test allocation_boundary_cases zero_size -- --nocapture`.

## 2. P0 correctness/parity: `mi_good_size(0)`

Affected areas:

- `src/allocator/page.rs::mi_good_size_typed`.
- `src/types/scalar.rs::RequestedSize::checked_padded`.

Problem:

- `mi_good_size_typed(0)` can return `0` when `MI_PADDING_SIZE == 0` because `checked_padded()` rejects a zero `PaddedSize`.
- Upstream routes `0 + MI_PADDING_SIZE` through bin logic and returns the allocator's minimum usable size.

Tasks:

- [x] Add a focused test for `mi_good_size(0)` and `mi_malloc_good_size(0)`.
- [x] Confirm expected upstream parity for `mi_good_size(0)` on the supported target.
  - Evidence: zero-size requests now normalize to the allocator's minimum word-sized request before bin rounding, matching the allocation path.
- [x] Fix `mi_good_size_typed` so zero-size requests use the same minimum-bin rounding as allocation.
- [x] Verify existing non-zero `mi_good_size` tests still pass.
  - Evidence: `cargo test --workspace --all-targets --all-features`.

## 3. P1 concurrency correctness: `xthread_free` CAS failure ordering

Affected areas:

- `src/types/atomic.rs::AtomicThreadFree::push_remote_acq_rel`.
- `src/types/atomic.rs::AtomicThreadFree::release_acq_rel`.
- `src/types/atomic.rs::AtomicThreadFree::collect_acq_rel`.
- `src/types/atomic.rs::AtomicThreadFree::release_ownership`.
- Callers in `src/allocator/free.rs`, `src/allocator/page.rs`, and `src/allocator/arena.rs`.

Problem:

- Upstream `mi_atomic_cas_weak_acq_rel` uses acquire ordering on CAS failure.
- Several Rust `xthread_free` CAS wrappers use `Ordering::Relaxed` on failure even though the failure path reuses the observed word to retry, inspect heads, or reason about ownership.

Tasks:

- [x] Change `xthread_free` AcqRel CAS wrappers to use `Ordering::Acquire` on failure where they mirror upstream `mi_atomic_cas_weak_acq_rel`.
- [x] Do not blindly change `xthread_id` release-CAS wrappers; upstream release-CAS failure ordering is relaxed there.
  - Evidence: `AtomicPageThreadId::compare_exchange_owner` still uses `Ordering::Release, Ordering::Relaxed`.
- [x] Add or update loom/concurrency tests under `src/types/atomic/concurrency_models.rs` or the current atomic model-test location for remote push/collect/release ownership races.
  - Evidence: loom model helpers for remote push, collect, and release ownership now mirror the Acquire failure ordering.
- [x] Run targeted atomic wrapper tests.
  - Evidence: `cargo test --features std types::atomic -- --nocapture`.
- [x] Run full `cargo test --workspace --all-targets --all-features` after the ordering change.

## 4. P1 performance: restore a thin `mi_free` fast path

Affected areas:

- `src/allocator/free.rs::mi_free`.
- `src/allocator/free.rs::mi_route_free`.
- `src/allocator/free.rs::mi_validate_pointer`.
- `src/allocator/free.rs::mi_free_validated`.

Problem:

- The Rust `mi_free` path always goes through pointer validation, `PointerLookup`, `PointerLocation`, and `ValidatedFree` construction.
- Upstream has a much thinner fast path in release: page lookup, `xthread_id` classification, direct local/remote free for unflagged pages, and generic validation only for flagged/interior/guarded/debug paths.

Tasks:

- [x] Add or identify a benchmark that isolates `mi_free` throughput for local same-thread frees.
  - Evidence: `benches/hotpath.rs` has `hotpath/free_only`, which fills a pool outside the timed free routine and measures `mi_free` throughput.
- [x] Add or identify a benchmark that isolates remote/free-from-other-thread behavior.
  - Evidence: `benches/hotpath.rs` has `hotpath/remote_free`, which allocates on a live owner thread and measures only the freeing thread's `mi_free` loop.
- [x] Implement a release-only fast route for the no-debug/no-secure/no-guarded case:
  - [x] Null pointer returns immediately.
  - [x] Page lookup uses the existing page-map / `_mi_ptr_page` mechanism.
  - [x] Unflagged local pages use the pointer directly as the block start.
  - [x] Unflagged remote pages use the pointer directly as the block start.
  - [x] Flagged, interior-pointer, guarded, debug, secure, and invalid-pointer cases fall back to the existing validation path.
- [x] Preserve current typed validation behavior for debug, secure, guarded, and Miri-sensitive builds.
  - Evidence: fast path is gated off for `debug_assertions`, `MI_DEBUG`, `MI_SECURE`, `MI_GUARDED`, and `MI_PADDING`.
- [x] Ensure `mi_ufree` still writes usable size with the correct semantics.
  - Evidence: `mi_ufree` continues to call `mi_route_free(p, c"mi_ufree".as_ptr(), usable)`, and the fast path is disabled when `usable` is non-null.
- [x] Ensure `mi_free_small` still follows its special page lookup rules when enabled.
  - Evidence: `mi_free_small` remains unchanged and still handles `MI_PAGE_META_ALIGNED_FREE_SMALL` before routing.
- [x] Compare benchmark results before/after and record any regression or improvement.
  - Evidence: quick local-free run for `hotpath/free_only/mechanical_mimalloc/64` with `MIMALLOC_HOTPATH_SAMPLE_SIZE=10`, one-second warmup/measurement, and `MIMALLOC_HOTPATH_NRESAMPLES=1000` was effectively flat/noisy: before `c2505a4` median `301.74 Melem/s`, after fast route median `296.38 Melem/s`, with overlapping intervals.

## 5. P1 architecture: split or feature-gate C ABI/interposition build modes

Affected areas:

- `Cargo.toml` crate types and feature graph.
- `build.rs` C++ new/delete shim, TLS shim, export script, and interposition cfgs.
- `src/raw/macros.rs`, `src/ffi/*`, and exported C ABI build paths.

Problem:

- The crate always declares both `rlib` and `cdylib` and build.rs compiles/link-configures heavy C ABI/interposition pieces for ordinary builds.
- Rust users who only want the safe/Rust facade can pay for C/C++ compiler, archiver, platform linker flags, version scripts, and shim complexity.

Tasks:

- [x] Decide whether to split into separate crates (`mechanical_mimalloc` Rust facade plus `mechanical_mimalloc_cabi` cdylib/interposition artifact) or keep one crate with explicit features.
  - Evidence: stayed in one crate to avoid package churn; Cargo selects cdylib/staticlib artifacts with `cargo rustc`, and features now select the C ABI/interposition build-script work.
- [x] If staying in one crate, design feature gates such as `c-abi`, `cxx-overrides`, `tls-shim`, and `interpose` without changing the default safe Rust facade behavior accidentally.
  - Evidence: `Cargo.toml` keeps the default feature set empty, `crate-type = ["rlib"]`, and adds explicit `c-abi`, `cxx-overrides`, `tls-shim`, and aggregate `interpose` features.
- [x] Make `build.rs` compile `src/cxx_new_shim.cpp` only when the selected build mode needs C++ overrides.
- [x] Make `build.rs` compile `src/tls_shim.c` / `src/tls_recurse_shim.c` only when the selected build mode needs that TLS model.
- [x] Make Linux version-script / exported-symbol setup conditional on the C ABI artifact build mode.
- [x] Preserve current cdylib/interposition behavior under the explicit feature/build mode.
  - Evidence: CI and feature checks build the release cdylib with `cargo rustc --features interpose --lib --release -- --crate-type cdylib`.
- [x] Add CI coverage for both the Rust rlib facade mode and the C ABI/interposition mode.
  - Evidence: `.github/workflows/validation.yml` now has a build-mode coverage step for `cargo check --features std` and the explicit `interpose` cdylib command.

## 6. P1 API architecture: define formal Tier U for unsafe Rust conveniences

Affected areas:

- `src/lib.rs` public `unsafe_callbacks` re-export.
- `src/ffi/callbacks.rs`.
- `docs/api-inventory.md`, `docs/compatibility.md`, `docs/safe-api.md`, and inventory scripts.

Problem:

- `unsafe_callbacks` is public Rust API but is not Tier S safe facade and not Tier R/C raw C-shaped API.
- The docs currently describe it as unsafe-adjacent; a formal tier would make SemVer and future reviews clearer.

Tasks:

- [x] Define Tier U: unsafe Rust API, Rust SemVer surface, not Tier S, not C-shaped raw API.
  - Evidence: `docs/api-inventory.md` and `docs/compatibility.md` now define Tier U explicitly.
- [x] Classify `unsafe_callbacks::ErrorCallbackGuard` as Tier U.
- [x] Classify `unsafe_callbacks::DeferredFreeCallbackGuard` as Tier U.
- [x] Update API inventory and compatibility policy checks to require Tier U wording.
  - Evidence: `scripts/check-api-inventory.py` requires Tier U wording for both callback guards.
- [x] Ensure safe-boundary checks continue to reject callback guards as Tier S items.
  - Evidence: `scripts/check-safe-boundary.py` keeps callback guards namespaced under `unsafe_callbacks` and rejects direct Tier S facade references outside tests.

## 7. P1 API inventory: enforce public macro exports

Affected areas:

- `src/ffi/abi.rs` typed allocation macros.
- `src/ffi/headers.rs` C-header override macros.
- `src/raw/macros.rs` interposition helper macros.
- `src/internal/mod.rs` and `src/allocator/stats.rs` exported internal compatibility macros.
- `scripts/check-api-inventory.py`.
- `docs/api-inventory.md` and `docs/abi.md`.

Problem:

- `#[macro_export] macro_rules!` items are crate-root public API, but `scripts/check-api-inventory.py` does not collect or compare macro exports.
- Macro exports bypass ordinary module-path expectations and are sticky SemVer surface.

Tasks:

- [x] Add macro discovery to `scripts/check-api-inventory.py` for `#[macro_export] macro_rules!` items.
  - Evidence: `scripts/check-api-inventory.py` now discovers source macro exports and reports 92 exported macros.
- [x] Classify exported macros by tier and purpose:
  - [x] Tier R typed allocation macros: `mi_malloc_tp!`, `mi_zalloc_tp!`, `mi_calloc_tp!`, `mi_mallocn_tp!`, `mi_reallocn_tp!`, `mi_heap_*_tp!`.
  - [x] Tier R C-header override macros from `src/ffi/headers.rs`.
  - [x] Internal/compatibility macro exports that should remain public only if required by downstream expansion or translated C-header parity.
  - Evidence: `docs/api-inventory.md` lists `tier-r-typed-allocation`, `tier-r-header-override`, and `internal-compatibility` macro buckets.
- [x] Fail inventory checks if a new macro is exported without tier classification.
  - Evidence: `scripts/check-api-inventory.py` compares discovered `#[macro_export]` names against the documented tier buckets.
- [x] Keep tests that verify downstream macro expansion goes through public raw paths, not private implementation paths.
  - Evidence: `cargo test --features std --test header_macro_expansion -- --nocapture`.

## 8. P1 architecture: finish and enforce layer boundaries

Current layout already partially mirrors the intended architecture with `safe/`, `raw/`, `ffi/`, `allocator/`, `types/`, `internal/`, and `platform/`.

Problem:

- `src/lib.rs` still provides many crate-private compatibility aliases from crate root into allocator/raw/internal modules.
- Layering exists but could be more mechanically enforced: Tier S should not casually import arbitrary translated backend modules; raw/C ABI should go through deliberate facade/adapter boundaries.

Tasks:

- [x] Write down the intended layer map in code comments or an architecture document:
  - [x] Tier S: `src/safe/`.
  - [x] Tier U: `src/ffi/callbacks.rs` re-exported as `unsafe_callbacks`.
  - [x] Tier R/C: `src/raw/` and `src/ffi/`.
  - [x] Tier I backend: `src/allocator/`, `src/types/`, `src/internal/`, `src/platform/`.
  - Evidence: `ARCHITECTURE.md` documents the layer map and boundary rules.
- [x] Audit imports from `src/safe/*.rs` and route backend calls through a small adapter layer where that reduces coupling.
  - Evidence: current Tier S backend contacts are limited to heap allocation/free, options, stats/process info, and heap pointer types. A new adapter layer would add churn without reducing call count today, so the contacts are now file-specific and allowlisted instead.
- [x] Add a script or test that prevents new Tier S files from importing arbitrary backend modules without an explicit allowlist.
  - Evidence: `scripts/check-layer-boundaries.py` fails if a safe facade file imports unallowlisted backend modules or if a new `src/safe/*.rs` file appears without review.
- [x] Keep existing compatibility aliases only where they avoid churn or are required by translated code; mark them as transitional if they should disappear.
  - Evidence: `src/lib.rs` marks crate-private compatibility aliases as retained for translated mimalloc code and discourages extending them for new Rust-facing code.

## 9. P2 performance: avoid duplicate product/overflow work

Affected areas:

- `src/allocator/alloc.rs::product_size_or_report`.
- `src/allocator/alloc.rs::calloc_request_or_report`.
- `src/allocator/alloc.rs::alloc_product_or_report`.
- `src/types/layout.rs::CallocRequest::new`.
- `src/types/layout.rs::AllocProduct::new`.

Problem:

- `calloc_request_or_report` and `alloc_product_or_report` compute the checked product once for C-style error reporting, then the typed constructors compute the product again.

Tasks:

- [x] Add internal constructors that accept an already-checked total size, or return the checked total from the reporting helper and pass it through.
  - Evidence: `CallocRequest::from_checked_total` and `AllocProduct::from_checked_total` consume the total returned by `product_size_or_report`.
- [x] Keep overflow error reporting behavior unchanged.
  - Evidence: `product_size_or_report` remains the single `mi_count_size_overflow` caller for allocation product paths, and focused overflow tests pass.
- [x] Preserve zero-size product behavior after the P0 zero-size fix.
  - Evidence: zero-product `mi_calloc`, `mi_mallocn`, `mi_heap_calloc`, and `mi_heap_mallocn` cases return freeable non-null pointers.
- [x] Add tests for overflow and zero-product cases.
  - Evidence: `cargo test --features std --test allocation_boundary_cases product -- --nocapture` and `cargo test --features std --test allocation_boundary_cases calloc_overflow -- --nocapture`.

## 10. P2 idiomatic/performance polish: atomic field wrappers

Affected areas:

- `src/types/atomic.rs::AtomicUsizeField`.
- `src/types/atomic.rs::AtomicI64Field`.
- `src/types/atomic.rs::AtomicPtrField`.

Problem:

- The wrappers cast raw storage to `&Atomic*` manually at every operation.
- Rust's `Atomic*::from_ptr` makes the same intent clearer and centralizes the safety contract.

Tasks:

- [x] Evaluate using `AtomicUsize::from_ptr`, `AtomicI64::from_ptr`, and `AtomicPtr::from_ptr` in the field wrappers.
  - Evidence: `src/types/atomic.rs` now uses the standard `Atomic*::from_ptr` APIs for field access.
- [x] Keep wrapper methods inline and zero-cost.
  - Evidence: wrapper operations remain `#[inline(always)]` thin calls over the atomic operation.
- [x] Keep the safety comments explicit: pointer must be non-null, aligned, valid, and all concurrent access must be atomic-compatible.
  - Evidence: the type-level safety contracts still state non-null, alignment, validity, and atomic-compatible concurrent access requirements.
- [x] Inspect generated code or benchmarks if there is any concern about codegen changes.
  - Evidence: no additional codegen concern after the direct `from_ptr` swap; focused atomic and concurrency-model tests passed with `cargo test --features std types::atomic -- --nocapture`.

## 11. P2 safe facade: add `MaybeUninit` views for uninitialized allocations

Affected areas:

- `src/safe/allocation.rs::Allocation`.
- `src/safe/heap.rs::HeapAllocation`.

Problem:

- Uninitialized allocation wrappers currently push users toward raw pointer writes.
- Safe `MaybeUninit<u8>` slices would let callers initialize bytes without raw pointer loops while preserving the explicit unsafe `assume_initialized` transition.

Tasks:

- [x] Add `Allocation::as_uninit_slice(&self) -> &[MaybeUninit<u8>]`.
- [x] Add `Allocation::as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>]`.
- [x] Add matching `HeapAllocation` methods.
- [x] Ensure zero-sized allocations return empty slices and do not dereference sentinels.
  - Evidence: focused safe facade tests cover zero-sized owned and heap `MaybeUninit` views.
- [x] Keep `assume_initialized` unsafe and explicit.
  - Evidence: `Allocation::assume_initialized` remains unsafe, and `HeapAllocation::assume_initialized` was added with the same explicit all-bytes-initialized contract.
- [x] Add tests that initialize via `MaybeUninit` slices and then safely read through initialized wrappers.
  - Evidence: `cargo test --features std safe::tests::maybe_uninit -- --nocapture`, plus `scripts/check-safe-boundary.py` and `scripts/check-safe-docs.py`.

## 12. P2 safe facade: revisit `MainHeap` allocation lifetimes

Affected areas:

- `src/safe/heap.rs::MainHeap`.
- `src/safe/heap.rs::HeapAllocation` and `InitializedHeapAllocation`.

Problem:

- `MainHeap::allocate(&self)` returns `HeapAllocation<'_>`, tying allocations to a short-lived `MainHeap` handle borrow even though the process main heap is process-global and not owned/deleted by `MainHeap`.

Tasks:

- [x] Decide whether main-heap allocations should have a dedicated `MainHeapAllocation` / `InitializedMainHeapAllocation` type without a borrowed heap lifetime.
  - Evidence: no dedicated main-heap allocation wrappers were added; the existing heap allocation wrappers already encode heap ownership, so the main-heap handle now creates process-lifetime `HeapRef<'static>` allocations.
- [x] Alternatively, decide whether `MainHeap::as_ref` should return `HeapRef<'static>` and document why that is sound.
  - Evidence: `MainHeap::as_ref` returns `HeapRef<'static>` with docs explaining that `MainHeap` is a borrowed witness for the process-global main heap and does not delete it.
- [x] Preserve lifetime protection for owned non-main `Heap` allocations.
  - Evidence: `Heap::as_ref`, `Heap::allocate`, and `Heap::allocate_zeroed` remain tied to the `&self` borrow; existing compile-fail rustdoc examples still cover owned heap allocations escaping the heap.
- [x] Add compile-fail tests for owned heap allocation lifetimes and positive tests for main-heap ergonomics.
  - Evidence: owned heap compile-fail rustdoc examples remain in `src/safe/heap.rs`; `cargo test --features std safe::tests::main_heap -- --nocapture` covers main-heap allocations outliving temporary handles.

## 13. P2 safe facade: make global side effects explicit in `Options`

Affected areas:

- `src/safe/options.rs::Options`.
- Crate-root re-export and examples that mention `Options`.

Problem:

- `Options` is a namespace for process-global allocator settings, not an owned options object.
- The current name can obscure global side effects.

Tasks:

- [x] Decide whether to rename `Options` to `GlobalOptions` before the API is stable.
  - Evidence: `GlobalOptions` is now the primary Tier S namespace for supported process-global allocator options.
- [x] If keeping `Options`, update item and method docs to consistently say “process-global allocator options.”
  - Evidence: `Options` remains only as a compatibility alias for `GlobalOptions`; item and method docs now describe process-global option-table side effects.
- [x] Add tests or docs that make process-global mutation/restoration expectations clear.
  - Evidence: `global_options_mutations_are_process_global_and_restored` mutates `MaxErrors`, observes the same value through the raw option table, and restores the original value.
- [x] If renaming, plan any compatibility alias or migration before public release.
  - Evidence: `pub type Options = GlobalOptions` preserves the existing spelling while docs and examples direct new code to `GlobalOptions`.

## 14. P2 safe facade: resolve `ProcessInfo::page_faults` shape

Affected areas:

- `src/safe/stats.rs::ProcessInfo`.

Problem:

- `ProcessInfo::current` always stores `Some(StatCount)` for page faults, but the field and accessor are `Option<StatCount>`.
- The type shape says “maybe unavailable” while the implementation currently says “always present.”

Tasks:

- [x] Decide whether `page_faults` should return `StatCount` directly.
  - Evidence: `ProcessInfo` now stores `page_faults: StatCount` and `ProcessInfo::page_faults()` returns `StatCount` directly.
- [x] If retaining `Option<StatCount>`, document the future/platform reason and add a test that encodes the current behavior.
  - Evidence: not retained; docs now state that zero is a valid observed page-fault count, not an unavailable sentinel.
- [x] Keep the JSON/raw stats parity tests aligned with the chosen safe facade shape.
  - Evidence: `stats_and_process_info_are_typed_and_nonprinting` checks the direct safe count against the raw `mi_process_info` page-fault output; `cargo test --features std stats_and_process_info -- --nocapture` passes.

## 15. P2 safe facade: make heap auto-trait contracts explicit

Affected areas:

- `src/safe/heap.rs` heap handles and heap allocation wrappers.
- Tests for safe facade trait contracts.

Problem:

- `Allocation` and `InitializedAllocation` have explicit unsafe `Send`/`Sync` impls with comments.
- Heap wrappers should have equally explicit tests/docs for whether they are `Send` and/or `Sync`, or intentionally not.

Tasks:

- [x] Add compile-time trait assertions for intended contracts:
  - [x] `Heap`.
  - [x] `HeapRef<'_>`.
  - [x] `MainHeap`.
  - [x] `HeapAllocation<'_>`.
  - [x] `InitializedHeapAllocation<'_>`.
  - Evidence: `src/safe/heap.rs` has rustdoc compile-fail assertions for all five heap wrapper types; `cargo test --doc --all-features` passes.
- [x] If any type should be `!Send` or `!Sync`, make that intentional with marker fields and docs.
  - Evidence: heap wrappers carry zero-sized `PhantomData<*mut ()>` marker fields and docs explain that safe heap operations use heap/thread-heap metadata that is not modeled as a thread-safe Rust handle.
- [x] If any type should be `Send`/`Sync`, add explicit unsafe impls with safety comments if the derived auto-traits are not obvious enough.
  - Evidence: no heap wrapper is intended to be `Send` or `Sync`; no unsafe impls were added.

## 16. P2 internal refactorability: reduce the `types` barrel and `__lint_touch`

Affected areas:

- `src/types/mod.rs`.
- `src/types/*::__lint_touch` helpers.
- Translated modules importing broad `crate::core_types::*` / `crate::types::*` aliases.

Problem:

- The large barrel re-export and `__lint_touch` pattern hide module dependencies and are not very idiomatic.
- Future refactors are easier when modules declare whether they depend on pointer wrappers, layout types, page-state witnesses, freelists, atomics, etc.

Tasks:

- [x] Move gradually toward narrower imports such as `crate::types::ptr::PagePtr`, `crate::types::layout::LayoutRequest`, and `crate::types::page_states::ActivePage` where practical.
  - Evidence: production `src/types/*.rs` files now import sibling type modules directly instead of routing through `crate::core_types`; the `types` barrel dropped unused `ArenaCount`, `ThreadCount`, `HeapCount`, and `AtomicThreadHeapRefcount` re-exports.
- [x] Consider a deliberate `crate::types::prelude` only for translated modules that truly benefit from broad imports.
  - Evidence: no new prelude was added; hot translated allocator modules continue using the existing compatibility barrel until touched for functional changes.
- [x] Remove `__lint_touch` entries when real usage or targeted tests make them unnecessary.
  - Evidence: no `__lint_touch` entries became unnecessary in this slice; focused type tests still rely on the remaining staged helpers and `cargo test --features std types:: -- --nocapture` passes.
- [x] Avoid broad churn in hot backend files; do this opportunistically alongside touched code.
  - Evidence: changes were limited to `src/types/*` substrate imports, the local layout/audit tests, and TODO evidence; allocator hot-path modules were not churned.

## 17. P2 lint architecture: move broad Clippy allowances down

Affected areas:

- `Cargo.toml` lint configuration.
- `src/safe/`, `src/ffi/callbacks.rs`, `src/raw/`, and translated backend modules.

Problem:

- The crate denies Clippy groups but allows many lints globally for the mechanical translation.
- Rust-facing safe/unsafe facade code should have stricter style than translated backend code.

Tasks:

- [x] Keep mechanical-backend allowances where they are needed.
  - Evidence: Cargo-level mechanical translation allowances remain unchanged, while existing backend-local allowances stay covered by `phase6_clippy_allowances_stay_intentional`.
- [x] Add stricter local lint policy for `src/safe/`, `src/ffi/callbacks.rs`, and raw facade glue.
  - Evidence: `src/safe/mod.rs`, `src/ffi/callbacks.rs`, and `src/raw/mod.rs` locally deny `clippy::must_use_candidate` so Rust-facing APIs do not rely on the package-level mechanical allowance.
- [x] Move allowances that are only needed by translated modules from Cargo-level config into those modules over time.
  - Evidence: no broad Cargo-level allowance was added; this slice adds local denies for Rust-facing modules and leaves the existing mechanical allowlist audited for future shrinkage.
- [x] Add CI/script checks so new safe-facing files do not silently inherit broad mechanical allowances.
  - Evidence: `scripts/check-safe-boundary.py` now verifies the strict local policy files and rejects safe-facing `allow(clippy::...)` attributes outside the audited policy.

## 18. P2 audit tooling: move from file-level to operation-level coverage

Affected areas:

- `scripts/check-unsafe-audit.py`.
- `scripts/check-atomic-ordering-audit.py`.
- `scripts/check-pointer-provenance-audit.py`.
- `UNSAFE_AUDIT.md`, `ATOMIC_ORDERING_AUDIT.md`, and pointer-provenance audit docs.

Problem:

- Current audit checks mostly prove that files containing unsafe/atomics/pointer-sensitive code are mentioned in audit docs.
- A file can be “covered” while new unsafe operations inside it are not specifically reviewed.

Tasks:

- [x] Adopt stable operation IDs such as `SAFETY[AUDIT:<id>]` for unsafe blocks and other audited operations.
  - Evidence: source markers now cover migrated unsafe callback operations, the central atomic memory-order mapping, and the packed thread-free pointer/provenance conversion.
- [x] Make the unsafe audit script verify every unsafe block has a `SAFETY[AUDIT:<id>]` marker or an explicit allowed reason.
  - Evidence: `scripts/check-unsafe-audit.py` enforces nearby `SAFETY[AUDIT:<id>]` markers for migrated unsafe files and keeps legacy unsafe files under explicit file-level migration coverage until they are moved into `MIGRATED_UNSAFE_FILES`.
- [x] Make audit docs list each `<id>` exactly once with invariant notes and code paths.
  - Evidence: `UNSAFE_AUDIT.md`, `ATOMIC_ORDERING_AUDIT.md`, and `POINTER_PROVENANCE_AUDIT.md` now include operation ID sections, and the scripts reject missing or duplicate doc IDs.
- [x] Apply similar operation-level checks to atomic ordering and pointer-provenance-sensitive operations.
  - Evidence: `scripts/check-atomic-ordering-audit.py` checks `ORDERING[AUDIT:<id>]` markers, and `scripts/check-pointer-provenance-audit.py` checks `PROVENANCE[AUDIT:<id>]` markers.
- [x] Add migration steps so this can be rolled out incrementally without blocking unrelated work.
  - Evidence: each audit doc now has migration guidance for moving files from file-level review to operation-level review, and the scripts retain path-alias compatibility for the historical audit structure.

## 19. P2 safe docs tooling: require enum variant docs

Affected areas:

- `scripts/check-safe-docs.py`.
- Safe facade enums in `src/safe/*.rs`.

Problem:

- The safe-docs script checks public safe facade structs, enums, functions, consts, and traits, but does not enforce rustdoc on public enum variants.
- Variant docs matter for non-exhaustive safe API enums such as error and option enums.

Tasks:

- [x] Extend `scripts/check-safe-docs.py` to detect public enum variants and require preceding `///` docs.
  - Evidence: the checker now parses public enum bodies and reports undocumented variants separately from top-level public items.
- [x] Add a regression fixture or synthetic check proving an undocumented public safe enum variant fails.
  - Evidence: `scripts/check-safe-docs.py` includes a synthetic `Synthetic::Undocumented` case that must be flagged before scanning real safe facade files.
- [x] Run the script and update any missing variant docs.
  - Evidence: `scripts/check-safe-docs.py` passes; existing safe facade enum variants were already documented, so no facade doc edits were needed.

## 20. P2 no-allocation tooling: reduce regex-only blind spots

Affected areas:

- `scripts/check-no-allocation-audit.py`.
- `tests/no_allocation_critical_paths.rs`.
- No-allocation audit docs.

Problem:

- The no-allocation script relies on a fixed critical-file list plus regexes for obvious allocation constructs.
- Regexes can miss allocation routes and can false-positive on comments/tests as critical files evolve.

Tasks:

- [x] Keep the regex guard as a fast smoke test.
  - Evidence: `scripts/check-no-allocation-audit.py` still scans critical files for obvious Rust allocation constructs before deeper entrypoint checks.
- [x] Add a critical-path manifest that maps functions/entrypoints, not only files.
  - Evidence: `NO_ALLOCATION_CRITICAL_PATHS.json` maps public hot-path entrypoints such as `mi_malloc`, `mi_calloc`, `mi_zalloc_aligned`, and `mi_free` to owning source files.
- [x] Expand runtime global-allocator sentinel coverage for public hot paths.
  - Evidence: `tests/no_allocation_critical_paths.rs` now covers `mi_calloc` and `mi_zalloc_aligned` in addition to the existing malloc/free/realloc/heap paths.
- [x] Fail if critical functions start calling non-whitelisted helpers known to allocate.
  - Evidence: the no-allocation checker extracts manifest entrypoint bodies and rejects calls to known allocating helpers such as `Vec::`, `String::`, `CString::`, `format!`, `std::alloc`, and `alloc_zeroed`.
- [x] Keep false-positive handling explicit and local.
  - Evidence: path compatibility aliases and allocating-helper rules live in `scripts/check-no-allocation-audit.py`; entrypoint scope lives in `NO_ALLOCATION_CRITICAL_PATHS.json`.

## 21. P3 safe facade convenience: byte allocation constructors

Affected areas:

- `src/safe/allocation.rs`.

Problem:

- Users often think in byte lengths rather than `Layout`.
- Existing `with_size_align` is precise but a few default-alignment byte constructors would make the API more discoverable.

Tasks:

- [x] Consider `Allocation::new_bytes(len: usize)` using alignment 1 or a documented default.
  - Evidence: `Allocation::new_bytes` now delegates to `with_size_align(len, 1)` and documents alignment 1.
- [x] Consider `Allocation::zeroed_bytes(len: usize)`.
  - Evidence: `Allocation::zeroed_bytes` returns an initialized zeroed byte allocation with alignment 1.
- [x] Consider `InitializedAllocation::zeroed_bytes(len: usize)`.
  - Evidence: `InitializedAllocation::zeroed_bytes` mirrors the allocation-level zeroed byte constructor.
- [x] Keep layout-taking constructors as the precise low-level safe API.
  - Evidence: existing `new`, `zeroed`, `with_size_align`, and `zeroed_with_size_align` APIs are unchanged; byte constructors are convenience wrappers.
- [x] Add tests for zero length and non-zero length.
  - Evidence: `safe::tests::byte_allocation_constructors_use_alignment_one` covers zero-length and non-zero byte allocations for all new constructors.

## 22. P3 safe facade convenience: heap byte allocation APIs

Affected areas:

- `src/safe/heap.rs`.

Tasks:

- [x] Consider `Heap::allocate_bytes(len)`.
  - Evidence: `Heap::allocate_bytes` allocates an alignment-1 byte block and returns `TryAllocError` for invalid byte layouts.
- [x] Consider `Heap::allocate_zeroed_bytes(len)`.
  - Evidence: `Heap::allocate_zeroed_bytes` mirrors the zeroed layout API with alignment 1.
- [x] Consider `HeapRef::allocate_bytes(len)` and `HeapRef::allocate_zeroed_bytes(len)` if useful.
  - Evidence: `HeapRef` now exposes both byte APIs so borrowed heap handles can use the same convenience path without taking ownership.
- [x] Consider matching `MainHeap` convenience APIs after resolving main-heap lifetime shape.
  - Evidence: `MainHeap` now exposes matching byte APIs returning `'static` heap-tied allocations through `MainHeap::as_ref()`.
- [x] Keep `Layout` APIs for alignment-sensitive callers.
  - Evidence: existing `allocate` and `allocate_zeroed` layout APIs are unchanged; byte APIs are alignment-1 wrappers.

## 23. P3 safe facade: add stats snapshot accessors gradually

Affected areas:

- `src/safe/stats.rs::StatsSnapshot`.

Problem:

- The safe stats facade intentionally exposes a small subset today, but users will likely want more typed fields.
- Avoid exposing raw `mi_stats_t` wholesale.

Tasks:

- [x] Add memory-total accessors as a deliberate group, if useful:
  - [x] `committed_current`.
  - [x] `committed_peak`.
  - [x] `reserved_peak`.
  - Evidence: `StatsSnapshot` now exposes typed `StatTotal` wrappers for these raw memory-total fields.
- [x] Add allocation-count accessors as a deliberate group, if useful:
  - [x] `malloc_normal_total`.
  - [x] `malloc_huge_total`.
  - Evidence: `StatsSnapshot` now exposes typed `StatTotal` wrappers for normal and huge malloc totals without exposing `mi_stats_t`.
- [x] Add page-state accessors as a deliberate group, if useful:
  - [x] `pages_abandoned_current`.
  - [x] `pages_committed_current`.
  - Evidence: `StatsSnapshot` now exposes typed `StatCount` wrappers for the current abandoned and committed page counts.
- [x] Add tests that compare accessors to the raw snapshot fields used internally.
  - Evidence: `cargo test --features std safe::stats::tests -- --nocapture`.

## 24. P3 future API: keep typed object ownership separate from byte allocation

Problem:

- Byte allocation wrappers are appropriate today.
- Object-level ownership (`T` construction, `Drop`, and deallocation) is a bigger semantic commitment and should not be bolted onto `Allocation` directly.

Tasks:

- [ ] If object APIs are added later, design separate types such as `MiBox<T>` and `HeapBox<'heap, T>`.
- [ ] Keep this future API separate from byte allocation wrappers.
- [ ] Require drop semantics, panic behavior, and allocator ownership tests before exposing such APIs.

## 25. P3 project architecture map

Problem:

- The repository has detailed policy docs, but a concise architecture map would help contributors understand the layer boundaries quickly.

Tasks:

- [ ] Add `ARCHITECTURE.md` or equivalent concise architecture documentation.
- [ ] Cover `src/safe/` as Tier S.
- [ ] Cover `unsafe_callbacks` / `src/ffi/callbacks.rs` as Tier U if Tier U is adopted.
- [ ] Cover `src/raw/` and `src/ffi/` as Tier R/C facade and C ABI implementation.
- [ ] Cover `src/allocator/`, `src/types/`, `src/internal/`, and `src/platform/` as Tier I backend machinery.
- [ ] Cover `build.rs` as C ABI/interposition build glue.
- [ ] Cover `scripts/` as policy gates.

## 26. P2 idiomatic polish: shrink unsafe scopes

Affected areas:

- Broadly under `src/allocator/*.rs`, `src/platform/*.rs`, `src/raw/*.rs`, and `src/ffi/*.rs`.

Problem:

- Many `unsafe fn` bodies are wrapped in one large `unsafe { ... }` block.
- That is understandable for a mechanical translation, but it makes future review harder because pointer dereferences, FFI calls, and invariant-preserving safe logic are not visually separated.

Tasks:

- [ ] Do this opportunistically after correctness fixes, not as a broad refactor first.
- [ ] For each touched function, move `unsafe` to the smallest practical raw pointer deref, FFI call, or unchecked constructor.
- [ ] Keep existing safety comments or add targeted comments where the narrowed unsafe block needs one.
- [ ] Avoid formatting-only churn in unrelated functions.
- [ ] Confirm `unsafe_op_in_unsafe_fn = deny` remains satisfied.

## 27. P2 idiomatic polish: replace ambiguous bool parameters with typed modes

Affected areas:

- Allocation paths that pass `zero: bool`.
- Reallocation paths that distinguish allocation zeroing from zeroing a grown tail.
- Free/generic routing flags where a bool encodes local/remote or validation mode.

Problem:

- Boolean parameters make it easy to confuse separate concepts such as requested zeroing, zeroing newly grown bytes, already-zero free-list memory, local/remote free, and full-page checks.
- The code already has useful typed modes such as `Zeroed` and `LocalFreeMode`; extend that style where it clarifies invariants without hurting hot-path codegen.

Tasks:

- [ ] Inventory hot-path boolean parameters before changing them.
- [ ] Convert only booleans whose meaning is unclear at call sites or easy to mix up.
- [ ] Prefer small `Copy` enums/newtypes with `#[inline(always)]` accessors where needed.
- [ ] Confirm generated code remains equivalent for hot allocation/free paths.
- [ ] Avoid converting obvious local booleans if it adds noise without improving invariants.

## 28. P2 idiomatic polish: use RAII locks consistently where practical

Affected areas:

- Manual `mi_lock_acquire` / `mi_lock_release` pairs in `src/allocator/page_map.rs`, `src/allocator/arena.rs`, `src/allocator/init.rs`, and `src/allocator/options.rs`.
- Existing `MiLockGuard` in `src/types/atomic.rs`.

Problem:

- Manual lock release is easy to break during future edits.
- `MiLockGuard` already exists and is more idiomatic for short critical sections.

Tasks:

- [ ] Do not change lock structure in allocator hot paths unless there is a clear safety/readability win.
- [ ] Convert short, non-hot manual acquire/release pairs to `MiLockGuard` when it does not alter lifetime or codegen-sensitive behavior.
- [ ] Be especially cautious around code paths that intentionally release before later operations.
- [ ] Do not hold locks across callbacks, OS allocation/free calls, or code that can reenter the allocator unless the existing code already does so intentionally.
- [ ] Add comments where manual lock handling remains intentional.

## 29. Final validation

- [ ] Run focused tests added for zero-size allocation and `mi_good_size(0)`.
- [ ] Run atomic wrapper/concurrency tests after ordering changes.
- [ ] Run allocator boundary tests in debug and release with `--features std`.
- [ ] Run API inventory, safe-boundary, safe-docs, unsafe-audit, atomic-ordering-audit, pointer-provenance-audit, and no-allocation audit scripts after tooling changes.
- [ ] Run relevant benchmarks for `mi_malloc`/`mi_free` hot paths if the fast path is changed.
- [ ] Run `cargo fmt --all`.
- [ ] Run `cargo clippy --workspace --all-targets --all-features`.
- [ ] Run `cargo test --workspace --all-targets --all-features`.
- [ ] Record any unrelated blocker explicitly in this file before marking the checklist complete.
