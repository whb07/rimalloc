# TODO: Rust Polish Spec Implementation Checklist

Source of truth: `RUST_POLISH_SPEC.md`.

Scope: implement the Rust polish pass completely and faithfully. This is an execution checklist, not a second spec. Do not mark an item complete until the named code/docs change is made and the validation evidence is passing or an explicit unrelated-blocker note is recorded next to the item.

## 0. Baseline and work tracking

- [ ] Read `RUST_POLISH_SPEC.md` end-to-end before starting implementation.
- [ ] Record the starting commit hash and current branch in the implementation notes.
- [ ] Run `git status --short` and confirm the starting tree state is understood.
- [ ] Run the current audit scripts once to capture baseline failures: `scripts/check-unsafe-audit.py`, `scripts/check-atomic-ordering-audit.py`, `scripts/check-pointer-provenance-audit.py`, `scripts/check-no-allocation-audit.py`, `scripts/check-safe-boundary.py`, and `scripts/check-api-inventory.py`.
- [ ] Run the baseline Rust checks required by project policy: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features`, and `cargo test --workspace --all-targets --all-features`.
- [ ] Record every baseline failure that is unrelated to the polish pass so later validation can distinguish regressions from pre-existing failures.
- [ ] Implement sections in priority order unless a later section is required to unblock an earlier validation step.
- [ ] Keep each commit or work chunk scoped to one coherent section of this checklist where practical.

## 1. Audit and CI drift

- [ ] Complete this section only after all audit scripts pass locally and are required by CI/release checks.

### 1.1 Restore unsafe-audit script coverage

- [ ] Inspect `scripts/check-unsafe-audit.py` and list every source file or glob it currently treats as audited.
- [ ] Compare that script inventory with the current source tree under `src/`, including `src/safe/*.rs` and nested `src/core_types/*/tests.rs` files.
- [ ] Update `scripts/check-unsafe-audit.py` so every current unsafe-audited source file is intentionally included or intentionally excluded with a documented reason.
- [ ] Prefer robust discovery/globbing in `scripts/check-unsafe-audit.py` where possible so future module additions fail loudly instead of silently drifting.
- [ ] Add or update tests/checks in `scripts/check-unsafe-audit.py` so missing audit entries fail with actionable file names.
- [ ] Run `scripts/check-unsafe-audit.py` and fix every failure caused by stale inventory or missing audit references.

### 1.2 Restore atomic-ordering audit coverage

- [ ] Inspect `scripts/check-atomic-ordering-audit.py` and list every hard-coded file path or audit marker it expects.
- [ ] Compare the script expectations with current atomic usage in `src/atomic.rs`, allocator internals, safe facade modules, tests, and any translated modules that use atomics.
- [ ] Update `scripts/check-atomic-ordering-audit.py` to match the current source structure.
- [ ] Add failure output that identifies any atomic operation lacking an audit marker or documented policy.
- [ ] Run `scripts/check-atomic-ordering-audit.py` and fix every stale-path or missing-audit failure.

### 1.3 Restore pointer-provenance audit coverage

- [ ] Inspect `scripts/check-pointer-provenance-audit.py` and list every hard-coded file path or provenance marker it expects.
- [ ] Compare the script expectations with current pointer arithmetic, integer-pointer casts, raw allocation wrappers, and safe facade pointer escapes.
- [ ] Update `scripts/check-pointer-provenance-audit.py` to match the current source structure.
- [ ] Add failure output that identifies pointer-provenance-sensitive code without an audit marker or documented policy.
- [ ] Run `scripts/check-pointer-provenance-audit.py` and fix every stale-path or missing-audit failure.

### 1.4 Keep no-allocation audit authoritative

- [ ] Run `scripts/check-no-allocation-audit.py` and record current failures, if any.
- [ ] Inspect whether the no-allocation script depends on stale file paths, stale function names, or missing source modules.
- [ ] Update `scripts/check-no-allocation-audit.py` only as needed to reflect the current tree and intended no-allocation regions.
- [ ] Run `scripts/check-no-allocation-audit.py` again and confirm it exits 0.

### 1.5 Expand safe-boundary enforcement

- [ ] Inspect `scripts/check-safe-boundary.py` and document the exact files it currently scans.
- [ ] Expand `scripts/check-safe-boundary.py` to scan `src/lib.rs`.
- [ ] Expand `scripts/check-safe-boundary.py` to scan `src/safe.rs`.
- [ ] Expand `scripts/check-safe-boundary.py` to scan every Rust file under `src/safe/*.rs`.
- [ ] Make the safe-boundary script fail on public `unsafe fn` declarations in `src/safe/*.rs` unless the item is explicitly classified as non-Tier-S and allowed by path/name.
- [ ] Make the safe-boundary script fail on public unsafe constructors in safe facade module files, not only in `src/safe.rs`.
- [ ] Move any stale allowlist entry for `assume_initialized` from `src/safe.rs` to its actual module path, `src/safe/allocations.rs`, if that item remains allowed.
- [ ] Make every safe-boundary allowlist entry path-specific and item-specific.
- [ ] Add a regression fixture or synthetic check proving a new public unsafe function in `src/safe/*.rs` would fail the script.
- [ ] Run `scripts/check-safe-boundary.py` and fix all failures.

### 1.6 Add audit checks to CI and release docs

- [ ] Open `.github/workflows/validation.yml` and identify the job where repository scripts should run.
- [ ] Add `scripts/check-unsafe-audit.py` to CI validation.
- [ ] Add `scripts/check-atomic-ordering-audit.py` to CI validation.
- [ ] Add `scripts/check-pointer-provenance-audit.py` to CI validation.
- [ ] Add `scripts/check-no-allocation-audit.py` to CI validation.
- [ ] Add `scripts/check-safe-boundary.py` to CI validation if it is not already required.
- [ ] Ensure CI script steps use the same feature/toolchain assumptions documented for local validation.
- [ ] Update `docs/release.md` or the release checklist to require all audit scripts before publishing.
- [ ] Run the updated audit scripts locally in the same order CI will run them.

## 2. Reclassify unsafe callback guards

- [ ] Complete this section only after borrowed `FnMut` callback guards live outside Tier S safe API, constructors are infallible unless a real failure path exists, and docs/tooling enforce the classification.

### 2.1 Inventory current callback API exposure

- [ ] Inspect `src/safe/callbacks.rs` and list every public callback type, function, error, trampoline, and test.
- [ ] Inspect `src/safe.rs` for callback module declarations and re-exports.
- [ ] Inspect `src/lib.rs` for crate-root callback re-exports and crate-level docs that mention callback guards.
- [ ] Inspect `README.md`, `docs/safe-api.md`, `docs/api-inventory.md`, and rustdoc examples for claims that callback guards are safe facade APIs.
- [ ] Inspect `scripts/check-api-inventory.py` for hard-coded callback guard or `CallbackError` expectations.
- [ ] Inspect callback tests for `.expect("guard")` or other assumptions that constructors return `Result`.

### 2.2 Create the unsafe callback namespace

- [ ] Add a public Rust-facing unsafe-adjacent namespace named `unsafe_callbacks`.
- [ ] Put the RAII borrowed callback guard implementation under the `unsafe_callbacks` namespace rather than under the safe facade namespace.
- [ ] Keep C-shaped callback registration functions in `raw`; do not move raw ABI functions into `unsafe_callbacks`.
- [ ] Remove borrowed guard re-exports from `src/safe.rs` and the `safe` module public surface.
- [ ] Remove crate-root re-exports that present `ErrorCallbackGuard`, `DeferredFreeCallbackGuard`, or `CallbackError` as Tier S peers.
- [ ] If any crate-root callback export remains, rename or document it so it is visibly unsafe-adjacent and not part of Tier S.
- [ ] Update module paths in all internal tests and examples after moving the guards.
- [ ] Ensure `scripts/check-safe-boundary.py` no longer sees these borrowed guard constructors as safe-facade public unsafe functions.

### 2.3 Make callback guard constructors infallible

- [ ] Change `ErrorCallbackGuard::new(callback: &'a mut F)` from returning `Result<Self, CallbackError>` to returning `Self`, unless a real registration failure path is implemented.
- [ ] Change `DeferredFreeCallbackGuard::new(callback: &'a mut F)` from returning `Result<Self, CallbackError>` to returning `Self`, unless a real registration failure path is implemented.
- [ ] Remove `Ok(Self { ... })` wrapping from both constructors after the return type changes.
- [ ] Remove `CallbackError` if no remaining real fallible callback API uses it.
- [ ] Remove `CallbackError::Register` tests if registration still cannot fail.
- [ ] Remove `CallbackError::Unsupported` tests if unsupported registration still cannot occur through this API.
- [ ] Update callback tests that currently call `.expect("guard")` to bind guards directly.
- [ ] Update doctests and examples that expect callback guard constructors to return `Result`.
- [ ] Update public API inventory after the signature and error-type changes.

### 2.4 Preserve callback restoration and panic containment

- [ ] Verify `ErrorCallbackGuard` still snapshots the previous global error callback function and argument before installing the guard callback.
- [ ] Verify `DeferredFreeCallbackGuard` still snapshots the previous global deferred-free callback function and argument before installing the guard callback.
- [ ] Verify `Drop` for `ErrorCallbackGuard` restores the exact previous callback state.
- [ ] Verify `Drop` for `DeferredFreeCallbackGuard` restores the exact previous callback state.
- [ ] Keep or add tests that install nested/sequential error callback guards and confirm restoration order.
- [ ] Keep or add tests that install nested/sequential deferred-free callback guards and confirm restoration order.
- [ ] Preserve trampoline panic containment for `std` and test builds.
- [ ] Keep or add a child-process panic test for callback panic behavior if existing coverage is insufficient.
- [ ] Confirm no trampoline allows a Rust panic to unwind across the C ABI boundary.

### 2.5 Strengthen callback safety docs

- [ ] Add `# Safety` docs to `ErrorCallbackGuard::new` in the `unsafe_callbacks` namespace if they are not already present.
- [ ] Add `# Safety` docs to `DeferredFreeCallbackGuard::new` in the `unsafe_callbacks` namespace if they are not already present.
- [ ] Document that callback slots are process-global.
- [ ] Document that callbacks may be invoked from another thread unless upstream evidence proves otherwise.
- [ ] Document that callbacks may be invoked reentrantly unless upstream evidence proves otherwise.
- [ ] Document that callers must synchronize registration, restoration, and callback entry with all allocator activity that could invoke the callback.
- [ ] Document that the borrowed closure must outlive every possible callback invocation.
- [ ] Document that callers must not leak the guard.
- [ ] Document that callers must not call `core::mem::forget` on the guard.
- [ ] Document that callers must not wrap the guard in `ManuallyDrop` unless they manually restore the previous callback state before borrowed state can be invalidated.
- [ ] Document that preventing `Drop` can leave a process-global C callback pointing at borrowed Rust state.
- [ ] Document callback panic behavior for `std`, tests, and `no_std` if behavior differs.
- [ ] Document allocator reentry restrictions for callback bodies.

### 2.6 Update callback docs and inventories

- [ ] Update `README.md` so callback guards are not listed as part of the safe Rust facade.
- [ ] Update crate-root rustdoc so callback guards are not presented beside Tier S safe allocation, heap, options, stats, or process-info types.
- [ ] Update `docs/safe-api.md` to remove callback guards from the current safe surface.
- [ ] Update `docs/safe-api.md` to point unsafe callback users to `unsafe_callbacks` with a clear warning.
- [ ] Update `docs/api-inventory.md` to remove callback guards from Tier S.
- [ ] Update `docs/api-inventory.md` to inventory `unsafe_callbacks` as unsafe-adjacent public API if it is public.
- [ ] Update `scripts/check-api-inventory.py` so it no longer expects `CallbackError`, `DeferredFreeCallbackGuard`, or `ErrorCallbackGuard` as crate-root Tier S items.
- [ ] Add inventory-script expectations for the `unsafe_callbacks` module if it is public.
- [ ] Search the repository for stale phrases such as `callback guards` near `safe facade` and update every incorrect claim.

### 2.7 Defer truly safe callback API design

- [ ] Do not retrofit the borrowed `FnMut` guards into a safe Tier S API during this pass.
- [ ] If a future safe callback API is documented, mark it explicitly as future work and separate it from the current implementation.
- [ ] Ensure any future safe callback sketch uses `Fn`, not `FnMut`.
- [ ] Ensure any future safe callback sketch requires `'static` callback lifetime.
- [ ] Ensure any future safe callback sketch requires `Sync` for possible concurrent invocation.
- [ ] Ensure any future safe callback sketch avoids RAII restoration of arbitrary previous raw callback state.
- [ ] Ensure any future safe callback sketch specifies a global lock, once-cell, or equivalent synchronization model.
- [ ] Ensure any future safe callback sketch documents whether registration must happen before allocator concurrency starts or is internally synchronized.

### 2.8 Validate callback reclassification

- [ ] Run `cargo test --features std callbacks`.
- [ ] Run `scripts/check-safe-boundary.py`.
- [ ] Run `scripts/check-api-inventory.py`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Confirm generated rustdoc shows callback guards under `unsafe_callbacks`, not under the Tier S safe facade.

## 3. Rustdoc, stale docs, and examples

- [ ] Complete this section only after Tier S safe facade docs are complete and stale public API claims are corrected.

### 3.1 Inventory missing safe-facade docs

- [ ] Run `RUSTDOCFLAGS='-W missing-docs' cargo doc --no-deps --features std` and capture missing-docs output.
- [ ] Separate missing docs for Tier S safe facade items from staged raw/ABI missing docs.
- [ ] List every crate-root safe facade type that lacks item-level rustdoc.
- [ ] List every public safe facade method that lacks item-level rustdoc.
- [ ] List every public safe facade enum variant that lacks item-level rustdoc.
- [ ] List every public safe facade accessor that lacks item-level rustdoc.

### 3.2 Document allocation facade items

- [ ] Add rustdoc for `Allocation` if any public item-level docs are missing.
- [ ] Add rustdoc for every public `Allocation` method, including pointer, length, layout, initialization, resize, and ownership-transfer methods.
- [ ] Add rustdoc for `InitializedAllocation` if any public item-level docs are missing.
- [ ] Add rustdoc for every public `InitializedAllocation` method, including slice, pointer, clone/copy, resize, and deallocation behavior.
- [ ] Add rustdoc for allocation error types and every public variant/accessor.
- [ ] Ensure allocation docs describe safety-sensitive raw pointer escape hatches accurately after pointer accessor changes.

### 3.3 Document heap facade items

- [ ] Add rustdoc for `Heap` if any public item-level docs are missing.
- [ ] Add rustdoc for every public `Heap` method.
- [ ] Add rustdoc for `HeapRef` if any public item-level docs are missing.
- [ ] Add rustdoc for every public `HeapRef` method.
- [ ] Add rustdoc for `MainHeap` if any public item-level docs are missing.
- [ ] Add rustdoc for every public `MainHeap` method.
- [ ] Add rustdoc for `HeapAllocation` if any public item-level docs are missing.
- [ ] Add rustdoc for every public `HeapAllocation` method.
- [ ] Add rustdoc for `InitializedHeapAllocation` if any public item-level docs are missing.
- [ ] Add rustdoc for every public `InitializedHeapAllocation` method.
- [ ] Add rustdoc for heap error types and every public variant/accessor.

### 3.4 Document options facade items

- [ ] Add rustdoc for `OptionKey` and every public variant.
- [ ] Document whether `OptionKey` is deliberately exhaustive and limited to the supported safe option set.
- [ ] Add rustdoc for every `OptionKey` conversion or raw-code accessor.
- [ ] Add rustdoc for `Options`.
- [ ] Add rustdoc for every public `Options` method after `Options::get` is made infallible.
- [ ] Add rustdoc for option error types and every public variant/accessor that remains after removing impossible errors.

### 3.5 Document stats and process-info facade items

- [ ] Add rustdoc for `StatsSnapshot`.
- [ ] Add rustdoc for every public `StatsSnapshot` method and accessor.
- [ ] Add rustdoc for `StatCount`.
- [ ] Add rustdoc for every public `StatCount` constructor, conversion, and accessor.
- [ ] Add rustdoc for `StatTotal`.
- [ ] Add rustdoc for every public `StatTotal` constructor, conversion, and accessor.
- [ ] Add rustdoc for stats error types and every public variant/accessor.
- [ ] Add rustdoc for `ProcessInfo`.
- [ ] Add rustdoc for every public `ProcessInfo` accessor, including any duration-returning accessors added in this pass.

### 3.6 Correct stale docs and examples

- [ ] Update `README.md` to match the final Tier S safe facade inventory.
- [ ] Update `README.md` to remove callback guards from the safe facade list.
- [ ] Update `docs/final-api-surface-review.md` so crate-root public items match the current implementation.
- [ ] Update `docs/safe-api.md` so it matches the final Tier S surface.
- [ ] Update `docs/api-inventory.md` so it matches the final crate-root, safe, raw, and unsafe-adjacent surfaces.
- [ ] Update `examples/raw_heap.rs` to remove stale claims that no safe heap facade exists.
- [ ] Update `examples/raw_options.rs` to remove stale claims that no safe options facade exists.
- [ ] Update `examples/raw_stats.rs` to remove stale claims that no safe stats facade exists.
- [ ] Add or update a safe heap example using `Heap` if examples do not already cover it.
- [ ] Add or update a safe options example using `Options` if examples do not already cover it.
- [ ] Add or update a safe stats example using `StatsSnapshot` if examples do not already cover it.
- [ ] Search docs and examples for stale phrases such as `no safe Heap`, `no safe Options`, `no safe StatsSnapshot`, and `callback guards` near `safe facade`.
- [ ] Fix every stale search result or record why it remains correct.

### 3.7 Improve doctest coverage and docs checks

- [ ] Review ignored doctests in safe facade rustdoc.
- [ ] Convert ignored safe facade doctests to compiling doctests where allocator behavior is deterministic enough.
- [ ] Keep examples ignored only when deterministic execution is not practical, and document why.
- [ ] Add a script or extend an existing docs script to fail on missing docs for crate-root Tier S items.
- [ ] Add the docs script check for `src/safe.rs`.
- [ ] Add the docs script check for every file under `src/safe/*.rs`.
- [ ] Allow staged raw/ABI missing docs only through an explicit path or module allowlist.
- [ ] Run `cargo doc --no-deps --features std`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Run the narrower Tier S missing-docs script and confirm it passes.
- [ ] Run `RUSTDOCFLAGS='-W missing-docs' cargo doc --no-deps --features std` and either make it pass globally or record the remaining staged raw-only failures.

## 4. Make `no_std` behavior feature/target driven

- [ ] Complete this section only after `no_std` behavior is no longer accidentally profile-dependent or the remaining profile dependence is explicitly documented and validated.

### 4.1 Inventory current `no_std` configuration

- [ ] Inspect the crate-level `#![cfg_attr(..., no_std)]` expression in `src/lib.rs`.
- [ ] Inspect `Cargo.toml` features related to `std`, default features, and allocator runtime behavior.
- [ ] Inspect docs that mention `no_std`, including README, platform docs, release docs, and reproducibility docs.
- [ ] Run `cargo check --no-default-features` and record whether it currently uses `std` in debug builds.
- [ ] Run `cargo build --release --no-default-features` and record whether it currently uses `no_std` in optimized Linux builds.
- [ ] Identify any code that only compiles in debug no-default builds because `std` is still enabled.

### 4.2 Remove profile-dependent `no_std` gating

- [ ] Replace the current `not(debug_assertions)`-dependent `no_std` gate with feature-driven gating where supported.
- [ ] Prefer `#![cfg_attr(not(feature = "std"), no_std)]` if the target strategy supports it.
- [ ] If Linux-only `no_std` support is required, keep target gating but remove `not(debug_assertions)`.
- [ ] Fix compilation errors exposed by debug `no_std` builds without reintroducing profile-dependent `std`.
- [ ] Add explicit `cfg(feature = "std")` gates around code that truly requires `std`.
- [ ] Add `alloc` or `core` alternatives where appropriate instead of depending on `std`.
- [ ] Update docs to describe the final `std` and `no_std` feature behavior.
- [ ] Update release docs to require both debug and release no-default validation.

### 4.3 Validate `no_std` behavior

- [ ] Run `cargo check --no-default-features`.
- [ ] Run `cargo build --release --no-default-features`.
- [ ] Run `cargo check --features std`.
- [ ] Add CI coverage for `cargo check --no-default-features`.
- [ ] Add CI coverage for `cargo build --release --no-default-features` on the maintained `no_std` target.
- [ ] Confirm docs no longer describe profile-dependent `no_std` behavior unless that behavior is intentionally retained with a recorded allocator-runtime rationale.

## 5. Package contents and repo-only tooling

- [ ] Complete this section only after published package contents no longer advertise broken repo-only scripts or local aliases.

### 5.1 Inventory package contents

- [ ] Run `cargo package --list --allow-dirty` and save the current package file list.
- [ ] Identify every packaged file under `/scripts/**`.
- [ ] For each packaged script, list every file, directory, audit document, local config, and external tool it requires.
- [ ] Identify packaged scripts that require un-packaged inputs such as `mimalloc-3.3.2/`, root audit files, or repository-only setup files.
- [ ] Identify README or docs commands that assume local files not present in the package, including `.cargo/config.toml` aliases.

### 5.2 Exclude repo-only scripts from crates.io package

- [ ] Update `Cargo.toml` package include/exclude configuration so repo-only scripts are not packaged unless all inputs are also packaged and tested.
- [ ] Keep library source, examples, benches, docs, license files, build inputs, and essential metadata packaged.
- [ ] Ensure package exclusion does not remove files required by `build.rs` or by normal downstream builds.
- [ ] Update `scripts/check-package.sh` so it fails if repo-only scripts are accidentally packaged again.
- [ ] Update `scripts/check-package.sh` so it fails if package-required build inputs are accidentally excluded.

### 5.3 Fix README/tooling alias mismatch

- [ ] Search README and docs for `cargo clippy-strict`.
- [ ] Replace package-facing mentions of `cargo clippy-strict` with the full `cargo clippy --workspace --all-targets --all-features` command unless `.cargo/config.toml` is intentionally packaged.
- [ ] If `.cargo/config.toml` remains repo-only, document aliases as maintainer-only convenience commands.
- [ ] Ensure all user-facing validation commands work from a published package checkout.

### 5.4 Validate package contents

- [ ] Run `cargo package --list --allow-dirty` again and inspect the file list manually.
- [ ] Run `scripts/check-package.sh`.
- [ ] Confirm no packaged script depends on un-packaged repo-only files.
- [ ] Confirm README commands shown to package users do not require un-packaged aliases.

## 6. Safe facade pointer accessors

- [ ] Complete this section only after safe allocation wrappers use const/mut pointer separation and all call sites are updated.

### 6.1 Inventory pointer-returning safe facade methods

- [ ] Inspect `Allocation::as_ptr` and record its current signature and call sites.
- [ ] Inspect `InitializedAllocation::as_ptr` and record its current signature and call sites.
- [ ] Inspect `HeapAllocation::as_ptr` and record its current signature and call sites.
- [ ] Inspect `InitializedHeapAllocation::as_ptr` and record its current signature and call sites.
- [ ] Search `src/safe*.rs`, `tests/`, `examples/`, and docs for `as_ptr()` calls that assume `*mut u8` from shared `&self`.

### 6.2 Change allocation pointer APIs

- [ ] Change `Allocation::as_ptr(&self)` to return `*const u8`.
- [ ] Add `Allocation::as_mut_ptr(&mut self) -> *mut u8`.
- [ ] Decide whether uninitialized allocation wrappers need an additionally named raw-mutable escape hatch such as `as_uninit_ptr`, and implement/document it if needed.
- [ ] Change `InitializedAllocation::as_ptr(&self)` to return `*const u8`.
- [ ] Add `InitializedAllocation::as_mut_ptr(&mut self) -> *mut u8`.
- [ ] Update all allocation tests to use `as_mut_ptr` only when they have mutable access.
- [ ] Update all allocation docs and examples to show the const/mut distinction.

### 6.3 Change heap allocation pointer APIs

- [ ] Change `HeapAllocation::as_ptr(&self)` to return `*const u8`.
- [ ] Add `HeapAllocation::as_mut_ptr(&mut self) -> *mut u8`.
- [ ] Decide whether heap uninitialized allocation wrappers need an additionally named raw-mutable escape hatch such as `as_uninit_ptr`, and implement/document it if needed.
- [ ] Change `InitializedHeapAllocation::as_ptr(&self)` to return `*const u8`.
- [ ] Add `InitializedHeapAllocation::as_mut_ptr(&mut self) -> *mut u8`.
- [ ] Update all heap allocation tests to use `as_mut_ptr` only when they have mutable access.
- [ ] Update all heap allocation docs and examples to show the const/mut distinction.

### 6.4 Validate pointer accessor changes

- [ ] Run `cargo test --features std safe::tests`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Run `scripts/check-api-inventory.py`.
- [ ] Search for `as_ptr(&self) -> *mut u8` and confirm no safe facade method still exposes that shape.

## 7. Rename `try_clone_zeroed_contents`

- [ ] Complete this section only after the misleading zeroed-content naming is removed from public safe facade docs/tests/API inventory.

### 7.1 Rename the method

- [ ] Locate `InitializedAllocation::try_clone_zeroed_contents`.
- [ ] Confirm the implementation allocates a new allocation and copies bytes from the existing initialized allocation.
- [ ] Rename `try_clone_zeroed_contents` to `try_clone_contents`.
- [ ] Update every internal call site to use `try_clone_contents`.
- [ ] Update every test to use `try_clone_contents`.
- [ ] Update every doctest and example to use `try_clone_contents`.
- [ ] Update docs to explain why cloning remains fallible rather than implementing infallible `Clone`.
- [ ] Remove the old method name before stabilization unless compatibility requirements are explicitly recorded.

### 7.2 Validate clone naming changes

- [ ] Run `cargo test --features std safe::tests`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Run `scripts/check-api-inventory.py`.
- [ ] Search the repository for `try_clone_zeroed_contents` and confirm no public docs or code still use it.

## 8. Make infallible APIs infallible

- [ ] Complete this section only after safe facade APIs no longer return `Result` without a real failure path.

### 8.1 Change `Options::get`

- [ ] Inspect `Options::get(key: OptionKey) -> Result<c_long, OptionError>` and confirm it cannot fail for a validated closed `OptionKey`.
- [ ] Change `Options::get` to return `c_long` directly.
- [ ] Remove `Ok(...)` wrapping from the implementation.
- [ ] Update every call site that currently uses `?`, `.unwrap()`, `.expect()`, or matches on `Options::get`.
- [ ] Update option tests to assert direct values instead of `Result` values.
- [ ] Update option docs and examples to show direct return behavior.
- [ ] Remove any `OptionError` variants that only existed to support impossible `Options::get` failures.
- [ ] Keep `OptionKey::from_raw(raw) -> Result<Self, OptionError>` fallible.
- [ ] Keep `Options::set` fallible if immutable or unsupported keys remain possible through the public type.

### 8.2 Confirm callback constructors are covered

- [ ] Confirm `ErrorCallbackGuard::new` returns `Self` after section 2.
- [ ] Confirm `DeferredFreeCallbackGuard::new` returns `Self` after section 2.
- [ ] Confirm no placeholder `CallbackError` remains unless a real fallible callback API remains.

### 8.3 Search for other false `Result` APIs

- [ ] Search safe facade modules for public functions returning `Result`.
- [ ] For each public safe facade `Result`, document the real failure path in rustdoc or change it to an infallible return type.
- [ ] Update tests and API inventory for every changed signature.

### 8.4 Validate infallible API changes

- [ ] Run `cargo test --features std safe::tests`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Run `scripts/check-api-inventory.py`.

## 9. Review public enums for future growth

- [ ] Complete this section only after every public safe facade enum is either deliberately exhaustive or marked `#[non_exhaustive]`.

### 9.1 Inventory public enums

- [ ] Inventory `TryAllocError` and record whether future variants are plausible.
- [ ] Inventory `ResizeErrorKind` and record whether future variants are plausible.
- [ ] Inventory `HeapError` and record whether future variants are plausible.
- [ ] Inventory `OptionError` and record whether future variants are plausible.
- [ ] Inventory `StatsError` and record whether future variants are plausible.
- [ ] Inventory `CallbackError` only if it remains after callback reclassification.
- [ ] Inventory `OptionKey` and record whether it deliberately models a closed supported safe option set.
- [ ] Search for any additional public safe facade enums and add them to the inventory.

### 9.2 Apply enum compatibility decisions

- [ ] Mark public error enums `#[non_exhaustive]` when more error variants are plausible.
- [ ] Keep `OptionKey` exhaustive only if the crate deliberately commits to the current supported safe option set.
- [ ] Remove unused or speculative enum variants before release rather than preserving placeholders.
- [ ] Update internal matches if `#[non_exhaustive]` changes external doctest matching patterns.
- [ ] Update rustdoc for every enum to state whether downstream exhaustive matching is intended.
- [ ] Update docs examples to include wildcard arms for non-exhaustive enums when matching outside the defining crate.

### 9.3 Validate enum changes

- [ ] Run `cargo test --features std`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Run `scripts/check-api-inventory.py`.

## 10. Stats and process-info type polish

- [ ] Complete this section only after stats signedness and process time units are explicit at the safe API boundary.

### 10.1 Decide and document `StatCount` signedness

- [ ] Inspect mimalloc stats sources to determine whether public count values can be negative or represent signed deltas.
- [ ] Inspect translated stats code to see where `i64` values enter `StatCount`.
- [ ] Record the `StatCount` domain decision in rustdoc and docs.
- [ ] If public counts are never meant to be negative, change `StatCount` to a nonnegative representation or add a validating constructor.
- [ ] If signed values are meaningful, document `StatCount` as a signed statistic count/delta and explain when negative values can appear.
- [ ] Add tests covering the chosen `StatCount` negative-value behavior.
- [ ] Update docs examples to avoid implying signed stats are unsigned counts unless that is guaranteed.

### 10.2 Add duration-friendly process info accessors

- [ ] Inventory every `ProcessInfo` accessor that returns a raw millisecond `usize`.
- [ ] Decide names for duration-returning accessors using `core::time::Duration`.
- [ ] Add duration-returning accessors for elapsed time.
- [ ] Add duration-returning accessors for user time.
- [ ] Add duration-returning accessors for system time.
- [ ] Keep raw millisecond accessors only if C parity or compatibility is useful, and document them as raw millisecond values.
- [ ] Add tests for duration conversions, including zero and representative nonzero millisecond values.
- [ ] Update rustdoc and examples to prefer duration-returning accessors for Rust-facing code.

### 10.3 Validate stats/process polish

- [ ] Run `cargo test --features std safe::tests`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Run `scripts/check-api-inventory.py`.

## 11. Safe facade trait polish

- [ ] Complete this section only after unambiguous standard trait impls are added and broader `Deref` behavior is intentionally avoided.

### 11.1 Add byte-slice conversion traits

- [ ] Implement `AsRef<[u8]>` for `InitializedAllocation`.
- [ ] Implement `AsMut<[u8]>` for `InitializedAllocation`.
- [ ] Implement `AsRef<[u8]>` for `InitializedHeapAllocation`.
- [ ] Implement `AsMut<[u8]>` for `InitializedHeapAllocation`.
- [ ] Add tests that pass initialized allocation wrappers to functions accepting `AsRef<[u8]>`.
- [ ] Add tests that pass initialized allocation wrappers to functions accepting `AsMut<[u8]>`.
- [ ] Do not add `Deref` unless a separate API review concludes the wrappers should behave like slices in every context.
- [ ] Document the `AsRef` and `AsMut` impls in relevant type rustdoc.

### 11.2 Add option-key conversion traits

- [ ] Implement `TryFrom<i32>` for `OptionKey`.
- [ ] Ensure `TryFrom<i32>` delegates to the same validation as `OptionKey::from_raw`.
- [ ] Add tests for valid `OptionKey` raw values.
- [ ] Add tests for invalid `OptionKey` raw values.
- [ ] Update docs to show `TryFrom<i32>` usage where appropriate.

### 11.3 Improve error source chaining

- [ ] Inventory safe facade error types that wrap another error.
- [ ] Implement `std::error::Error::source` for wrapper errors when the `std` feature is enabled and a source error exists.
- [ ] Add tests that verify `source()` returns the wrapped error for wrapper errors.
- [ ] Add tests that verify `source()` returns `None` for leaf errors.
- [ ] Update rustdoc for wrapper errors to describe source chaining.

### 11.4 Validate trait polish

- [ ] Run `cargo test --features std safe::tests`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Run `scripts/check-api-inventory.py`.

## 12. Narrow broad Clippy and dead-code allowances

- [ ] Complete this section only after safe facade modules are free of broad mechanical-translation allowances and lint inventories cannot grow accidentally.

### 12.1 Inventory current allowances

- [ ] Inspect `Cargo.toml` for package-level Clippy allowances.
- [ ] Inspect `src/lib.rs` for crate-level lint allowances.
- [ ] Search `src/` for `allow(dead_code)`.
- [ ] Search `src/` for broad style allowances related to C naming, argument count, casts, missing docs, or mechanical translation.
- [ ] Classify each allowance as raw ABI/parity, translated internals, safe facade, tests, or obsolete.
- [ ] Record the allowance inventory in source comments, a lint-check script, or docs.

### 12.2 Constrain allowances to translated internals

- [ ] Remove broad allowances from safe facade modules unless a narrow item-level reason exists.
- [ ] Remove obsolete allowances that no longer suppress warnings.
- [ ] Move necessary mechanical-translation allowances to the narrowest module or item that requires them.
- [ ] Add a short reason near every remaining `allow(dead_code)`.
- [ ] Add a short reason near every remaining broad Clippy/style allowance.
- [ ] Ensure new Rust-facing code does not rely on package-level mechanical-translation allowances.

### 12.3 Strengthen lint regression checks

- [ ] Update `scripts/check-lints.sh` so it fails if safe facade modules acquire broad dead-code or style allowances.
- [ ] Update `scripts/check-lints.sh` so it fails if the allowance inventory grows without an intentional update.
- [ ] Add or update tests that assert translated-internal allowance inventory does not grow accidentally.
- [ ] Run `scripts/check-lints.sh`.
- [ ] Run `cargo clippy --workspace --all-targets --all-features`.
- [ ] Run `cargo test --test release_fast_path_audit --features unstable-raw-internals` if that test remains part of the allowance inventory gate.

## 13. Toolchain and CI consistency

- [ ] Complete this section only after declared Rust version, CI toolchain, and platform tier docs agree.

### 13.1 Align Rust version policy

- [ ] Inspect `rust-toolchain.toml`.
- [ ] Inspect `Cargo.toml` `rust-version`.
- [ ] Inspect `.github/workflows/validation.yml` for `dtolnay/rust-toolchain@stable` or other stable-channel usage.
- [ ] Decide whether CI should use the pinned toolchain, test declared MSRV explicitly, or do both.
- [ ] Update CI so the declared `rust-version` or pinned toolchain is actually tested.
- [ ] Keep optional stable/beta/nightly smoke jobs only if they are labeled as such.
- [ ] Update docs to state the Rust version policy and whether patch/minor MSRV bumps are allowed.

### 13.2 Align platform tier policy

- [ ] Inspect `docs/platforms.md`.
- [ ] Inspect `docs/reproducibility.md`.
- [ ] Inspect CI jobs for Linux x86_64 GNU, Linux aarch64, macOS, and Windows.
- [ ] Decide which platforms are maintained targets and which are smoke/non-tier targets.
- [ ] Rename CI jobs or add job descriptions so smoke platforms are not presented as fully maintained if they are not.
- [ ] Update `docs/platforms.md` with tier expectations and required evidence for each platform.
- [ ] Update `docs/reproducibility.md` if toolchain or platform support claims change.

### 13.3 Validate toolchain/platform updates

- [ ] Run the updated CI-equivalent local commands for the maintained target.
- [ ] Confirm docs and workflow names use the same maintained/smoke terminology.
- [ ] Confirm `Cargo.toml`, `rust-toolchain.toml`, CI, and docs no longer contradict one another.

## 14. Build script portability

- [ ] Complete this section only after `build.rs` either uses standard Rust ecosystem compiler discovery or documents its manual toolchain requirements as intentional.

### 14.1 Inventory current build requirements

- [ ] Inspect `build.rs` for manual invocations of `cc`, `c++`, `ar`, or platform-specific tools.
- [ ] Inspect `scripts/check-build-tools.sh` for documented required tools.
- [ ] Inspect feature-gated build paths for `track-valgrind`, `track-asan`, and any C/C++ shims.
- [ ] Inspect cross-compilation docs for compiler, archiver, and linker assumptions.
- [ ] Record why manual command construction exists today and whether precise ABI/link behavior depends on it.

### 14.2 Improve build tool integration

- [ ] Prefer adopting the `cc` crate for C shim builds unless manual commands are required for a documented reason.
- [ ] Prefer adopting the `cc` crate for C++ shim builds unless manual commands are required for a documented reason.
- [ ] If adopting `cc`, add it as a build dependency in `Cargo.toml`.
- [ ] If adopting `cc`, migrate compiler flag setup from manual commands into `cc::Build`.
- [ ] If adopting `cc`, preserve existing ABI, feature, and platform-specific build behavior.
- [ ] If retaining manual commands, document the exact required tools and environment variables in build/platform docs.
- [ ] If retaining manual commands, keep or expand `scripts/check-build-tools.sh` so missing tools fail clearly.
- [ ] Update cross-compilation docs to match the chosen build strategy.

### 14.3 Validate build portability changes

- [ ] Run `scripts/check-build-tools.sh`.
- [ ] Run `cargo check --features track-valgrind`.
- [ ] Run `cargo check --features track-asan`.
- [ ] Run a normal `cargo check --features std`.
- [ ] Confirm build output and linked artifacts remain compatible with existing ABI/layout checks.

## 15. Repository hygiene

- [ ] Complete this section only after `.gitignore` covers known generated/local artifacts without hiding source or release evidence.

### 15.1 Inventory untracked/generated artifacts

- [ ] Run `git status --short` and list current untracked files.
- [ ] Identify Python cache artifacts such as `__pycache__/` and `*.pyc`.
- [ ] Identify local dump files such as `dump.rdb`.
- [ ] Identify benchmark scratch directories or generated benchmark data that should not be versioned.
- [ ] Identify editor, OS, and temporary files that commonly appear in the repository.
- [ ] Identify generated reports that are intentionally versioned and must not be ignored.

### 15.2 Update `.gitignore`

- [ ] Add ignores for Python caches if they are not intentionally versioned.
- [ ] Add ignores for local dump files if they are not intentionally versioned.
- [ ] Add ignores for benchmark scratch artifacts only when they are generated and not release evidence.
- [ ] Add ignores for editor/OS temporary files.
- [ ] Avoid broad patterns that could hide source files, audit evidence, benchmark reports, or package metadata.
- [ ] Add comments in `.gitignore` where a pattern might otherwise be ambiguous.

### 15.3 Validate repository hygiene

- [ ] Run `git status --short` and confirm only intentional files are shown.
- [ ] Run `cargo package --list --allow-dirty` and confirm package contents are unaffected by ignore-only changes.

## 16. Cross-cutting public API inventory

- [ ] Complete this section only after public API inventory tooling matches the final crate surface.

### 16.1 Update inventory after every API change

- [ ] Update inventory expectations after callback guard reclassification.
- [ ] Update inventory expectations after pointer accessor signature changes.
- [ ] Update inventory expectations after `try_clone_contents` rename.
- [ ] Update inventory expectations after `Options::get` return type change.
- [ ] Update inventory expectations after enum `#[non_exhaustive]` or variant removals.
- [ ] Update inventory expectations after stats/process-info accessor changes.
- [ ] Update inventory expectations after new standard trait impls.
- [ ] Ensure crate-root inventory distinguishes Tier S safe items, raw ABI items, and unsafe-adjacent items.

### 16.2 Validate API inventory

- [ ] Run `scripts/check-api-inventory.py`.
- [ ] Confirm the script fails if callback guards are reintroduced as crate-root Tier S peers.
- [ ] Confirm the script fails if safe facade items are removed from inventory without an intentional update.
- [ ] Confirm docs inventory and script inventory use the same classification terms.

## 17. Final verification for the polish pass

- [ ] Complete this section only after all prior sections are complete.

### 17.1 Formatting, linting, and tests

- [ ] Run `cargo fmt --all`.
- [ ] Run `git diff --check`.
- [ ] Run `cargo clippy --workspace --all-targets --all-features`.
- [ ] Run `cargo test --workspace --all-targets --all-features`.
- [ ] If any command fails for a reason unrelated to this work, record the exact command, failure summary, and follow-up item before marking the validation item complete.

### 17.2 Documentation validation

- [ ] Run `cargo doc --no-deps --features std`.
- [ ] Run `cargo test --doc --features std`.
- [ ] Run the Tier S missing-docs script added in section 3.
- [ ] Run `RUSTDOCFLAGS='-W missing-docs' cargo doc --no-deps --features std` and confirm it passes globally or only fails for explicitly staged raw/ABI docs.
- [ ] Search for stale callback-safe-facade claims and confirm none remain.
- [ ] Search for stale `no safe Heap`, `no safe Options`, and `no safe StatsSnapshot` claims and confirm none remain.

### 17.3 Script validation

- [ ] Run `scripts/check-api-inventory.py`.
- [ ] Run `scripts/check-safe-boundary.py`.
- [ ] Run `scripts/check-raw-facade.sh`.
- [ ] Run `scripts/check-ffi-boundary.py`.
- [ ] Run `scripts/check-layout.sh`.
- [ ] Run `scripts/check-abi-symbols.sh`.
- [ ] Run `scripts/check-features.sh`.
- [ ] Run `scripts/check-package.sh`.
- [ ] Run `scripts/check-docs.sh` if it exists or after creating/updating it.
- [ ] Run `scripts/check-unsafe-audit.py`.
- [ ] Run `scripts/check-atomic-ordering-audit.py`.
- [ ] Run `scripts/check-pointer-provenance-audit.py`.
- [ ] Run `scripts/check-no-allocation-audit.py`.
- [ ] Run `scripts/check-lints.sh`.
- [ ] Run `scripts/check-build-tools.sh`.

### 17.4 Feature and package validation

- [ ] Run `cargo check --no-default-features`.
- [ ] Run `cargo build --release --no-default-features`.
- [ ] Run `cargo check --features std`.
- [ ] Run `cargo package --list --allow-dirty` and inspect the result.
- [ ] Confirm package contents match README/package-user commands.

### 17.5 Final review

- [ ] Review the full diff and confirm every changed line traces to `RUST_POLISH_SPEC.md` or this checklist.
- [ ] Confirm no unrelated refactors, formatting churn, or cleanup outside the polish scope were included.
- [ ] Confirm safe facade modules remain warning-clean and do not rely on broad mechanical-translation allowances.
- [ ] Confirm unsafe borrowed callback guards are not classified as Tier S anywhere in code, docs, tests, or inventory scripts.
- [ ] Confirm the final implementation notes list all commands run and their outcomes.
