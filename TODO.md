# TODO: Rust Allocator Tightening Plan

Scope: turn the allocator review findings into small, verifiable actions. Each item names the location to change and includes a validation step. Do not mark an item complete until its validation passes or the confirmation note is recorded.

## 1. Re-audit Safe Callback Guards For Soundness

### 1.1 Document the actual callback threading contract

- [x] Inspect upstream mimalloc callback behavior for error callbacks.
  - Places: `mimalloc-3.3.2/include/mimalloc.h`, `mimalloc-3.3.2/src/options.c`, translated `src/options.rs`.
  - Action: determine whether an error callback may run on any thread, while another callback invocation is active, or during allocator reentry.
  - Validation: add a short conclusion to `docs/unsafe-audit.md` with the upstream source locations used.

- [x] Inspect upstream mimalloc callback behavior for deferred-free callbacks.
  - Places: `mimalloc-3.3.2/include/mimalloc.h`, `mimalloc-3.3.2/src/page.c`, translated `src/page.rs`.
  - Action: determine whether deferred-free callbacks may run cross-thread, concurrently, or reentrantly.
  - Validation: add a short conclusion to `docs/unsafe-audit.md` with the upstream source locations used.

- [x] Decide whether borrowed `FnMut` callback guards can remain safe.
  - Places: `src/safe/callbacks.rs`, `docs/safe-api.md`, `docs/unsafe-audit.md`.
  - Action: record one decision: keep safe with proven single-thread/non-reentrant behavior, add synchronization and bounds, or make constructors unsafe.
  - Validation: decision is stated in docs and referenced by tests or safety comments.

### 1.2 Tighten `ErrorCallbackGuard`

- [x] Fix the `ErrorCallbackGuard<'a, F: FnMut(i32)>` safety model.
  - Place: `src/safe/callbacks.rs` around `ErrorCallbackGuard` and `error_callback_trampoline`.
  - Action: either require safe concurrency bounds and synchronization, or make construction unsafe with explicit caller obligations.
  - Validation: `cargo test --quiet --features std safe::callbacks` passes, or equivalent callback tests pass.

- [x] Prevent unsynchronized concurrent mutation of error callbacks.
  - Place: `src/safe/callbacks.rs`.
  - Action: if the API remains safe, ensure the trampoline cannot obtain simultaneous mutable access to the same closure.
  - Validation: add a test or documented proof that concurrent callback entry cannot create two active `&mut F` borrows.

- [x] Preserve panic containment for error callbacks after the safety-model change.
  - Place: `src/safe/callbacks.rs`.
  - Action: keep `catch_unwind` behavior for `std`/test builds or document abort behavior for `no_std`.
  - Validation: existing child-process panic test still passes, or add/update one if needed.

- [x] Update `ErrorCallbackGuard` docs to match the final contract.
  - Places: `src/safe/callbacks.rs`, `docs/safe-api.md`.
  - Action: document thread, reentry, lifetime, restoration, and panic behavior.
  - Validation: `cargo test --doc --features std` passes.

### 1.3 Tighten `DeferredFreeCallbackGuard`

- [x] Fix the `DeferredFreeCallbackGuard<'a, F: FnMut(bool, u64)>` safety model.
  - Place: `src/safe/callbacks.rs` around `DeferredFreeCallbackGuard` and `deferred_free_callback_trampoline`.
  - Action: either require safe concurrency bounds and synchronization, or make construction unsafe with explicit caller obligations.
  - Validation: callback registration/unregistration tests pass under `cargo test --quiet --features std`.

- [x] Prevent unsynchronized concurrent mutation of deferred-free callbacks.
  - Place: `src/safe/callbacks.rs`.
  - Action: if the API remains safe, ensure concurrent allocator activity cannot call the same `FnMut` without synchronization.
  - Validation: add a test, stress test, or documented proof covering concurrent callback entry.

- [x] Preserve restoration of previous deferred-free callback state.
  - Places: `src/safe/callbacks.rs`, `src/page.rs` callback registration helpers.
  - Action: ensure `Drop` restores the exact previous function and argument after any API change.
  - Validation: add/keep a test that installs two guards in sequence and confirms restoration order.

- [x] Update `DeferredFreeCallbackGuard` docs to match the final contract.
  - Places: `src/safe/callbacks.rs`, `docs/safe-api.md`.
  - Action: document thread, reentry, lifetime, restoration, and panic behavior.
  - Validation: `cargo test --doc --features std` passes.

## 2. Restore Explicit Unsafe Boundaries In Exported Raw Macros

### 2.1 Typed allocation macros in `src/abi.rs`

- [x] Remove internal `unsafe` from `mi_malloc_tp!`.
  - Place: `src/abi.rs` macro `mi_malloc_tp!`.
  - Action: expand to the unsafe raw call without wrapping it in an internal unsafe block.
  - Validation: add or update compile-fail coverage proving caller code must use an explicit unsafe block.

- [x] Remove internal `unsafe` from `mi_zalloc_tp!`.
  - Place: `src/abi.rs` macro `mi_zalloc_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

- [x] Remove internal `unsafe` from `mi_calloc_tp!`.
  - Place: `src/abi.rs` macro `mi_calloc_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

- [x] Remove internal `unsafe` from `mi_mallocn_tp!`.
  - Place: `src/abi.rs` macro `mi_mallocn_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

- [x] Remove internal `unsafe` from both `mi_reallocn_tp!` arms.
  - Place: `src/abi.rs` macro `mi_reallocn_tp!`.
  - Action: require caller-side unsafe for both supported argument orders.
  - Validation: compile-fail coverage rejects both safe invocation forms.

- [x] Remove internal `unsafe` from `mi_heap_malloc_tp!`.
  - Place: `src/abi.rs` macro `mi_heap_malloc_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

- [x] Remove internal `unsafe` from `mi_heap_zalloc_tp!`.
  - Place: `src/abi.rs` macro `mi_heap_zalloc_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

- [x] Remove internal `unsafe` from `mi_heap_calloc_tp!`.
  - Place: `src/abi.rs` macro `mi_heap_calloc_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

- [x] Remove internal `unsafe` from `mi_heap_mallocn_tp!`.
  - Place: `src/abi.rs` macro `mi_heap_mallocn_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

- [x] Remove internal `unsafe` from `mi_heap_reallocn_tp!`.
  - Place: `src/abi.rs` macro `mi_heap_reallocn_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

- [x] Remove internal `unsafe` from `mi_heap_recalloc_tp!`.
  - Place: `src/abi.rs` macro `mi_heap_recalloc_tp!`.
  - Action: require caller-side unsafe at the macro invocation site.
  - Validation: compile-fail coverage rejects safe invocation.

### 2.2 Documentation and tests for macro unsafe policy

- [x] Add a positive example for caller-side unsafe macro use.
  - Places: `src/abi.rs` docs or `examples/ffi.rs`.
  - Action: show typed macro invocation inside an explicit unsafe block.
  - Validation: doctest or example build passes.

- [x] Add a negative example for safe macro invocation.
  - Place: crate-level docs or raw facade docs.
  - Action: add `compile_fail` coverage showing raw typed macros cannot be invoked from safe code.
  - Validation: `cargo test --doc --features std` passes and the compile-fail test fails for the intended reason.

- [x] Run raw facade inventory after macro changes.
  - Place: repository scripts.
  - Action: confirm macro changes do not alter the intended raw item inventory.
  - Validation: `scripts/check-raw-facade.sh` exits 0.

## 3. Fix Exported Header Override Macros That Reference Private Internals

### 3.1 Inventory every exported header macro

- [x] List all `#[macro_export]` macros in `src/headers.rs`.
  - Place: `src/headers.rs`.
  - Action: create an inventory grouped by target API: allocation, aligned allocation, POSIX, C++ operator shim, or unsupported/internal-only.
  - Validation: add the inventory to `docs/api-inventory.md` or a nearby source comment.

- [x] For each `src/headers.rs` macro, classify whether it is intended public Rust API.
  - Places: `src/headers.rs`, `docs/api-inventory.md`.
  - Action: mark each macro as public compatibility surface, internal parity-only, or candidate for removal/feature gating.
  - Validation: inventory names every exported macro exactly once.

### 3.2 Route public macros through public raw paths

- [x] Change `malloc!` to use a public raw path.
  - Place: `src/headers.rs` macro `malloc!`.
  - Action: replace `$crate::alloc::mi_malloc` with `$crate::raw::mi_malloc` if the macro remains public.
  - Validation: downstream smoke crate can invoke the macro from an explicit unsafe block.

- [x] Change `realloc!` to use a public raw path.
  - Place: `src/headers.rs` macro `realloc!`.
  - Action: route through `$crate::raw::mi_realloc`.
  - Validation: downstream smoke crate builds.

- [x] Change string/allocation macros to use public raw paths or gate them.
  - Places: `strdup!`, `_strdup!`, `realpath!`, `_wcsdup!`, `_dupenv_s!` in `src/headers.rs`.
  - Action: route through `raw` when a stable public raw function exists; otherwise hide behind an internal/unstable feature.
  - Validation: compile-fail coverage rejects private-module expansion from a downstream crate.

- [x] Change POSIX allocation macros to use public raw paths or gate them.
  - Places: `reallocarray!`, `posix_memalign!`, `cfree!`, `valloc!`, `memalign!` in `src/headers.rs`.
  - Action: route through `raw` when intended public; otherwise hide behind an internal/unstable feature.
  - Validation: downstream smoke crate builds for supported public macros.

- [x] Change aligned allocation macros to use public raw paths or gate them.
  - Places: `_expand!`, `_recalloc!`, `_aligned_recalloc!`, `_aligned_malloc!`, `_aligned_free!`, `_aligned_offset_realloc!` in `src/headers.rs`.
  - Action: route through `raw` when intended public; otherwise hide behind an internal/unstable feature.
  - Validation: downstream smoke crate builds for supported public macros.

- [x] Change C++ operator macros to use public raw paths or gate them.
  - Places: `operatornew!`, `operatornew_array!`, `operatornew_nothrow!`, `operatornew_array_nothrow!`, and `operatordelete*` macros in `src/headers.rs`.
  - Action: route through `raw` when intended public; otherwise hide behind an internal/unstable feature.
  - Validation: downstream smoke crate builds for supported public macros.

### 3.3 Confirm macro export behavior

- [x] Add downstream macro expansion tests.
  - Place: `tests/` or a package smoke-test script.
  - Action: create a temporary external crate that imports and expands representative public macros.
  - Validation: smoke test passes without referencing private modules.

- [x] Add negative coverage for gated/private macros.
  - Place: docs compile-fail or package smoke test.
  - Action: prove macros not intended as public API are not available by default.
  - Validation: compile-fail test rejects default use.

- [x] Re-run API inventory after header macro changes.
  - Place: repository scripts.
  - Action: confirm public macro boundary matches the documented inventory.
  - Validation: `scripts/check-api-inventory.py` exits 0.

## 4. Narrow Or Justify `Send` And `Sync` On Raw Allocator Types

### 4.1 Inventory existing unsafe impls

- [x] Inventory every `unsafe impl Send` in `src/types.rs`.
  - Place: `src/types.rs`.
  - Action: list each type, why it needs `Send`, and which safe wrapper or internal path depends on it.
  - Validation: inventory appears in `docs/unsafe-audit.md` or source comments.

- [x] Inventory every `unsafe impl Sync` in `src/types.rs`.
  - Place: `src/types.rs`.
  - Action: list each type, why it needs `Sync`, and which safe wrapper or internal path depends on it.
  - Validation: inventory appears in `docs/unsafe-audit.md` or source comments.

### 4.2 Review raw allocator state types individually

- [x] Review `mi_tld_s` `Send`/`Sync`.
  - Place: `src/types.rs` around `mi_tld_s`.
  - Action: prove thread-local data is safe to move/share, or remove the impls.
  - Validation: if retained, add safety comments explaining aliasing and thread-affinity invariants; if removed, tests still compile.

- [x] Review `mi_page_s` `Send`/`Sync`.
  - Place: `src/types.rs` around `mi_page_s`.
  - Action: prove page state is safe to move/share, or remove the impls.
  - Validation: if retained, add safety comments covering locks/atomics and raw pointer ownership; if removed, tests still compile.

- [x] Review `mi_theap_s` `Send`/`Sync`.
  - Place: `src/types.rs` around `mi_theap_s`.
  - Action: prove thread heap state is safe to move/share, or remove the impls.
  - Validation: if retained, add safety comments covering thread ownership and synchronization; if removed, tests still compile.

- [x] Review `mi_heap_s` `Send`/`Sync`.
  - Place: `src/types.rs` around `mi_heap_s`.
  - Action: prove heap state is safe to move/share, or remove the impls.
  - Validation: if retained, add safety comments covering cross-thread frees and internal locking; if removed, tests still compile.

- [x] Review `mi_arena_s`, `mi_arena_pages_s`, and `mi_subproc_s` `Send`/`Sync`.
  - Place: `src/types.rs` around arena/subproc impls.
  - Action: prove synchronization is sufficient, or remove the impls.
  - Validation: if retained, add safety comments covering global locks and atomics; if removed, tests still compile.

- [x] Review `mi_tls_slot_t` and `mi_thread_locals_t` `Send`/`Sync`.
  - Place: `src/types.rs` around TLS structs.
  - Action: prove these raw TLS mirror types are safe to move/share, or remove the impls.
  - Validation: if retained, add safety comments; if removed, tests still compile.

- [x] Review `mi_option_desc_t` `Send`/`Sync`.
  - Place: `src/types.rs` around option descriptor impls.
  - Action: prove shared option descriptors are immutable or synchronized, or remove the impls.
  - Validation: if retained, add safety comments; if removed, tests still compile.

### 4.3 Prefer typed wrappers over raw type auto-traits

- [x] Move concurrency guarantees to wrapper types where possible.
  - Places: `src/core_types/*.rs`, `src/safe/*.rs`, `src/types.rs`.
  - Action: remove raw C-layout auto-trait impls that are only needed by safe wrappers, and implement `Send`/`Sync` on the wrapper with narrower invariants.
  - Validation: `cargo test --quiet --features std` passes.

- [x] Add a compile-time check for intentional raw auto-traits.
  - Place: `tests/` or `src/core_types/*/tests.rs`.
  - Action: add assertions for the types intentionally `Send`/`Sync`, and avoid assertions for types intentionally not shareable.
  - Validation: test fails if an intentional auto-trait decision changes accidentally.

- [x] Re-run thread/concurrency tests after auto-trait changes.
  - Places: `tests/thread_stats_regression.rs`, `tests/original_test_stress.rs`, concurrency stress tests.
  - Action: verify allocator still supports intended cross-thread allocation/free patterns.
  - Validation: `cargo test --quiet --features std thread` and stress tests pass.

## 5. Reduce Crate-Wide Dead Code Suppression

### 5.1 Replace broad suppression with targeted allowances

- [x] Inventory dead-code warnings hidden by `#![allow(dead_code)]`.
  - Place: `src/lib.rs` crate attribute and all modules.
  - Action: temporarily remove the crate-wide allow locally and collect warnings without changing behavior.
  - Validation: save or summarize the warning inventory in an issue note or TODO subsection before editing.
  - Inventory from `cargo test --lib --features std` after removing the crate
    attribute: `src/alloc_aligned.rs` 2, `src/alloc_override.rs` 36,
    `src/arena.rs` 13, `src/arena_meta.rs` 1, `src/atomic.rs` 61,
    `src/bitmap.rs` 7, `src/bits.rs` 11,
    `src/core_types/freelist_wrappers.rs` 1, `src/init.rs` 6,
    `src/internal.rs` 38, `src/libc.rs` 5, `src/options.rs` 1,
    `src/page.rs` 9, `src/prim.rs` 12, `src/prim/unix.rs` 4,
    `src/provenance.rs` 4, `src/static.rs` 19, `src/stats.rs` 4,
    `src/theap.rs` 1, `src/track.rs` 6, `src/types.rs` 10.

- [x] Move dead-code allowances to mechanical translation modules that need them.
  - Places: likely `src/alloc.rs`, `src/page.rs`, `src/internal.rs`, `src/prim/*`, and other C-translation modules.
  - Action: replace crate-wide allowance with module-level `#[allow(dead_code)]` only where upstream parity requires retained unused code.
  - Validation: `cargo test --quiet --lib` emits no dead-code warnings.

- [x] Keep safe facade modules warning-clean without dead-code allowance.
  - Places: `src/safe.rs`, `src/safe/*.rs`.
  - Action: remove unused safe APIs or add tests/docs for intended public items rather than allowing dead code.
  - Validation: `cargo test --quiet --features std` emits no dead-code warnings from `src/safe*`.

- [x] Keep core type modules warning-clean where practical.
  - Places: `src/core_types/*.rs`.
  - Action: remove unused transitional helpers or add targeted allow comments explaining retained invariants.
  - Validation: `cargo test --quiet --lib` emits no dead-code warnings from `src/core_types*` unless explicitly justified.

### 5.2 Add regression checks for warning cleanliness

- [x] Add a check that routine test builds stay warning-free.
  - Place: existing scripts or a new warning check script.
  - Action: ensure warnings fail CI rather than being hidden by crate-wide allowances.
  - Validation: `scripts/check-lints.sh` or the new script exits nonzero on a synthetic warning.

- [x] Document why any remaining dead code is intentionally retained.
  - Places: source comments near allowances or `docs/upstream.md`.
  - Action: explain whether each retained unused item exists for ABI parity, platform parity, or future upstream translation.
  - Validation: source search for `allow(dead_code)` shows a nearby reason for each occurrence.

## 6. Make Raw Public Types More Opaque Where Possible

### 6.1 Inventory raw type exposure

- [x] Inventory types re-exported through `src/abi.rs`.
  - Place: `src/abi.rs` `pub use crate::types::{ ... }` block.
  - Action: list each exported raw type and whether C callers need the layout or only an opaque handle.
  - Validation: inventory is recorded in `docs/abi.md` or `docs/api-inventory.md`.

- [x] Inventory public fields on `repr(C)` raw structs.
  - Place: `src/types.rs`.
  - Action: identify which public fields are required for C-header compatibility and which could be hidden behind opaque aliases.
  - Validation: inventory names each externally visible `repr(C)` struct.

### 6.2 Opaque heap/thread handles

- [x] Review `mi_heap_t` exposure.
  - Places: `src/types.rs`, `src/abi.rs`, `src/raw.rs`.
  - Action: determine whether downstream Rust needs full `mi_heap_s` layout or only an opaque handle.
  - Validation: if changed, C ABI symbol tests and raw facade tests pass.

- [x] Review `mi_theap_t` exposure.
  - Places: `src/types.rs`, `src/abi.rs`, `src/raw.rs`.
  - Action: determine whether downstream Rust needs full `mi_theap_s` layout or only an opaque handle.
  - Validation: if changed, C ABI symbol tests and raw facade tests pass.

- [x] Review arena and subproc handle exposure.
  - Places: `src/types.rs`, `src/abi.rs`, `src/raw.rs`.
  - Action: determine whether `mi_arena_t`, `mi_subproc_t`, and related IDs can be opaque in public Rust while preserving ABI.
  - Validation: raw compatibility tests and docs examples still compile.

### 6.3 Preserve actual C layout compatibility

- [x] Keep internal layout tests for translated structs.
  - Places: `tests/compat_exports.rs`, layout tests, `src/core_types/layout*`.
  - Action: ensure hiding Rust fields does not remove internal layout verification.
  - Validation: `scripts/check-layout.sh` exits 0.

- [x] Add API compatibility tests for opaque-type decisions.
  - Place: `tests/release_fast_path_audit.rs` or API inventory script.
  - Action: assert public raw API exposes only intended type names and not unintended fields/modules.
  - Validation: `scripts/check-api-inventory.py` exits 0 and fails on an intentionally exposed field/module.

## 7. Add Targeted Unsafe-Audit Coverage

- [ ] Add an audit entry for every unsafe callback trampoline.
  - Places: `src/safe/callbacks.rs`, `docs/unsafe-audit.md`.
  - Action: document argument validity, lifetime, unwind, thread, and reentry invariants.
  - Validation: source search finds `SAFETY[AUDIT:safe-callback]` entries mirrored in `docs/unsafe-audit.md`.

- [ ] Add an audit entry for raw macro unsafe-boundary policy.
  - Places: `src/abi.rs`, `src/headers.rs`, `docs/abi.md`.
  - Action: document which macros require caller unsafe and which are intentionally safe wrappers, if any.
  - Validation: compile-fail tests align with the documented policy.

- [ ] Add an audit entry for retained raw auto-trait impls.
  - Places: `src/types.rs`, `docs/unsafe-audit.md`.
  - Action: document why each retained raw `Send`/`Sync` impl is sound.
  - Validation: every retained `unsafe impl Send` or `unsafe impl Sync` in `src/types.rs` has a nearby safety comment and audit reference.

## 8. Final Validation Before Closing This Tightening Pass

- [ ] Format the repository.
  - Action: run `cargo fmt --all`.
  - Validation: command exits 0 and leaves no uncommitted formatting changes beyond intentional edits.

- [ ] Run Clippy over the full workspace, targets, and features.
  - Action: run `cargo clippy --workspace --all-targets --all-features`.
  - Validation: command exits 0, or any unrelated failure is documented with a follow-up item.

- [ ] Run the full test matrix required by project policy.
  - Action: run `cargo test --workspace --all-targets --all-features`.
  - Validation: command exits 0, or any unrelated failure is documented with a follow-up item.

- [ ] Run API and boundary scripts after all tightening work.
  - Action: run `scripts/check-api-inventory.py`, `scripts/check-safe-boundary.py`, `scripts/check-ffi-boundary.py`, and `scripts/check-raw-facade.sh`.
  - Validation: all commands exit 0.

- [ ] Run layout, strict-type, feature, and ABI scripts after all tightening work.
  - Action: run `scripts/check-layout.sh`, `scripts/check-strict-types.py`, `scripts/check-features.sh`, and `scripts/check-abi-symbols.sh`.
  - Validation: all commands exit 0.

- [ ] Confirm documentation matches final API behavior.
  - Action: run documentation tests and search for stale claims about callback safety, raw macros, and public raw types.
  - Validation: `cargo test --doc --features std` exits 0 and stale wording search finds no mismatches.
