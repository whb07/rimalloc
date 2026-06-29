# Rust Polish Spec

Date: 2026-06-28

Scope: issues found while reviewing the translated Rust mimalloc 3.3.2 allocator for idiomatic Rust API shape, safety-boundary clarity, documentation quality, release hygiene, and maintainer expectations.

This spec treats the C-compatible `raw`/ABI layer as intentionally C-shaped. The goal is not to make that layer look like ordinary Rust; the goal is to ensure a Rust developer can clearly tell which parts are safe Rust API, which parts are raw compatibility surface, and which parts are internal translation machinery.

Status: this is a review/fix plan, not evidence that the fixes are already implemented.

## Guiding decisions

1. Keep C ABI compatibility explicit and separate from the Rust-facing API.
2. Do not hide unsafe global or callback obligations behind names that imply safe Rust.
3. Prefer small, staged fixes over redesigning allocator internals.
4. Make all policy claims testable in CI.
5. Keep the safe facade warning-clean and well documented even if raw internals retain mechanical-translation allowances.

## Priority order

1. Fix audit and CI drift.
2. Reclassify unsafe callback guards.
3. Fix stale docs/examples and Tier S rustdoc gaps.
4. Clarify `no_std`, package contents, and toolchain expectations.
5. Polish safe facade ergonomics.
6. Continue reducing broad lint/dead-code allowances in translated internals.

---

## 1. Audit and CI drift

### Issue

Several repository audit scripts no longer match the current source tree:

- `scripts/check-unsafe-audit.py` fails.
- `scripts/check-atomic-ordering-audit.py` fails.
- `scripts/check-pointer-provenance-audit.py` fails.

The safe-boundary script is also too narrow. It checks only:

- `src/lib.rs`
- `src/safe.rs`

It does not inspect `src/safe/*.rs`, where important safe-facade unsafe blocks, unsafe impls, and public unsafe constructors actually live.

CI and release docs do not currently require the unsafe/atomic/provenance audit scripts.

### Best fix

Make audit scripts authoritative again and run them in CI/release checks.

### Required changes

- Update `scripts/check-unsafe-audit.py` to cover the current file set, including `src/safe/*.rs` and nested `src/core_types/*/tests.rs` files if those remain intentionally audited.
- Update `scripts/check-atomic-ordering-audit.py` and `scripts/check-pointer-provenance-audit.py` to match current source structure.
- Expand `scripts/check-safe-boundary.py` to inspect:
  - `src/lib.rs`
  - `src/safe.rs`
  - every file under `src/safe/*.rs`
- Ensure the safe-boundary script detects public unsafe functions in module files, not only in `src/safe.rs`.
- Keep safe-boundary allowlists path- and item-specific when module scanning is added. For example, any allowance for `assume_initialized` must point at its actual module file, `src/safe/allocations.rs`, not stale `src/safe.rs` inventory.
- Make the script fail if a `src/safe/*.rs` module exposes a public `unsafe fn` without an explicit non-Tier-S classification.
- Add these scripts to `.github/workflows/validation.yml`:
  - `scripts/check-unsafe-audit.py`
  - `scripts/check-atomic-ordering-audit.py`
  - `scripts/check-pointer-provenance-audit.py`
  - `scripts/check-no-allocation-audit.py`
- Add the same audit checks to `docs/release.md` or the release checklist.

### Validation

```sh
scripts/check-unsafe-audit.py
scripts/check-atomic-ordering-audit.py
scripts/check-pointer-provenance-audit.py
scripts/check-no-allocation-audit.py
scripts/check-safe-boundary.py
```

---

## 2. Unsafe callback guards in the safe facade

### Issue

`ErrorCallbackGuard::new` and `DeferredFreeCallbackGuard::new` are public unsafe constructors with nontrivial global-concurrency obligations:

- `src/safe/callbacks.rs`
- process-global C callback slots
- possible cross-thread callback entry
- possible reentrant callback entry
- restoration of previous raw callback state
- borrowed `FnMut` closure state

They are marked `unsafe`, so this is not automatically unsound. But classifying these as Tier S safe API is confusing: the caller must uphold global process invariants that normal safe Rust APIs should not require.

The constructors also return `Result<_, CallbackError>`, but currently registration does not fail. `CallbackError` variants appear unused except as test/documentation placeholders.

### Best fix

Reclassify the current borrowed `FnMut` guards as unsafe/raw-adjacent APIs and make their constructors infallible.

Do not try to present the current scoped borrowed guards as fully safe Tier S API.

### Required changes

- Put these Rust-facing-but-unsafe RAII guards in one explicit namespace: `unsafe_callbacks`.
  - Keep C-shaped callback functions in `raw`.
  - Remove the borrowed guard re-exports from `src/safe.rs`/`safe`.
  - Remove crate-root re-exports that present the borrowed guard types as Tier S peers.
- Remove them from the Tier S safe API inventory.
- Update `README.md`, `docs/safe-api.md`, `docs/api-inventory.md`, and crate docs so they no longer imply these are safe facade APIs.
- Update `scripts/check-api-inventory.py` so its expected crate-root inventory no longer hard-codes `CallbackError`, `DeferredFreeCallbackGuard`, or `ErrorCallbackGuard` as safe/root items. If `unsafe_callbacks` is public, inventory it as an unsafe-adjacent module instead.
- Change constructors from:

```rust
pub unsafe fn new(callback: &'a mut F) -> Result<Self, CallbackError>
```

to:

```rust
pub unsafe fn new(callback: &'a mut F) -> Self
```

unless a real registration failure path is added.

- Remove `CallbackError`, or keep it only if a real fallible API uses it.
- Update callback tests that currently call `.expect("guard")` on the constructors; after the infallible change they should bind the guard directly.
- Remove or rewrite tests that only assert placeholder `CallbackError` variants exist unless a real fallible callback API remains.
- Keep the detailed `# Safety` docs on both constructors.
- Add an explicit safety requirement that callers must not leak, `mem::forget`, wrap in `ManuallyDrop`, or otherwise prevent `Drop` from running, because the process-global C callback slot may otherwise keep pointing at borrowed Rust state.
- Keep panic containment in the trampolines for `std`/test builds.

### Future safe callback API

If a truly safe callback API is needed later, add it separately instead of retrofitting these guards.

A safe API should be intentionally constrained, for example:

```rust
pub fn register_static_error_callback<F>(callback: &'static F)
where
    F: Fn(i32) + Sync + 'static;
```

Design constraints for any future safe callback API:

- use `Fn`, not `FnMut`;
- require `'static` callback lifetime;
- require `Sync` for possible concurrent invocation;
- do not use RAII to restore arbitrary previous raw callbacks;
- either install only once or restore only callbacks installed by the safe API itself;
- own the synchronization semantics with an explicit global lock/once-cell model;
- document whether registration must happen before allocator concurrency starts or is internally synchronized across registration, restoration, and callback entry;
- document callback panic behavior and allocator reentry behavior.

### Validation

```sh
cargo test --features std callbacks
scripts/check-safe-boundary.py
scripts/check-api-inventory.py
cargo test --doc --features std
```

---

## 3. Rustdoc and public API documentation gaps

### Issue

`RUSTDOCFLAGS='-W missing-docs' cargo doc --no-deps --features std` currently reports hundreds of missing documentation errors. Many are in the raw facade, which may be intentionally staged, but the Tier S safe facade also has missing docs.

Observed safe-facade gaps include:

- methods on `InitializedAllocation`;
- methods on `Heap`, `HeapRef`, `MainHeap`, `HeapAllocation`, and `InitializedHeapAllocation`;
- variants and accessors for error types;
- variants and methods for `OptionKey` and `Options`;
- methods and accessors on `StatsSnapshot`, `StatCount`, `StatTotal`, and `ProcessInfo`.

There is also stale documentation:

- `README.md` lists callback guards as part of the safe Rust facade.
- `docs/final-api-surface-review.md` lists an older, much smaller crate-root API.
- Some examples say there is no safe `Heap`, `Options`, or `StatsSnapshot` facade even though those now exist.

### Best fix

Add a Tier S documentation gate first. Do not block on documenting every raw ABI item immediately.

### Required changes

- Add item-level rustdoc for every crate-root safe facade type, method, enum variant, and accessor.
- Update stale docs that list old public API inventory, including `README.md`.
- Update examples with stale comments:
  - `examples/raw_heap.rs`
  - `examples/raw_options.rs`
  - `examples/raw_stats.rs`
- Consider adding safe examples beside raw examples:
  - heap allocation through `Heap`;
  - option read/set through `Options`;
  - stats snapshot through `StatsSnapshot`.
- Convert ignored doctests to compiling doctests where allocator behavior is deterministic enough.
- Add a script that fails on missing docs for `src/safe/*.rs` and crate-root safe items, while allowing staged raw docs.

### Validation

```sh
cargo doc --no-deps --features std
cargo test --doc --features std
RUSTDOCFLAGS='-W missing-docs' cargo doc --no-deps --features std
```

The last command may remain staged globally, but a narrower Tier S docs check should pass.

---

## 4. `no_std` behavior depends on profile

### Issue

The crate becomes `no_std` only under this condition:

```rust
all(
    not(test),
    target_os = "linux",
    not(debug_assertions),
    not(feature = "std")
)
```

This means debug `cargo check --no-default-features` still uses `std`, while optimized Linux builds without `std` use `no_std`. That profile-dependent behavior is unusual for Rust crates and can hide `no_std` regressions until release builds.

### Best fix

Make `no_std` feature/target driven, not profile driven, unless there is a documented allocator-specific reason that debug builds require `std`.

### Required changes

Preferred:

- Use `#![cfg_attr(not(feature = "std"), no_std)]` where supported by the target strategy.
- If only Linux `no_std` is supported, use target gating but avoid `not(debug_assertions)`.

If profile-dependent `no_std` must remain:

- document it prominently as an intentional allocator-runtime decision;
- add mandatory `cargo build --release --no-default-features` validation to local checks, CI, and release checks;
- add a direct check proving debug no-default builds are intentionally `std` builds.

### Validation

```sh
cargo check --no-default-features
cargo build --release --no-default-features
cargo check --features std
```

---

## 5. Package contents and repo-only scripts

### Issue

`Cargo.toml` packages `/scripts/**`, but many scripts depend on files that are not packaged, such as:

- `mimalloc-3.3.2/`
- root audit files like `UNSAFE_AUDIT.md`, `ATOMIC_ORDERING_AUDIT.md`, `POINTER_PROVENANCE_AUDIT.md`, and `NO_ALLOCATION_AUDIT.md`
- local repository setup files

The README also references the local Cargo alias `cargo clippy-strict`, but `.cargo/config.toml` is not included in the package.

### Best fix

Do not package repo-only scripts unless all of their inputs are also packaged and tested from the packaged crate.

### Required changes

Choose one:

1. Exclude repo-only scripts from the crates.io package and keep package contents focused on library, examples, benches, docs, and essential metadata.
2. Or include every input required by packaged scripts and add a package-consumer test that exercises the packaged scripts.

Preferred: option 1.

Also either:

- include `.cargo/config.toml` if README continues to mention `cargo clippy-strict`, or
- stop documenting the alias as a packaged-crate command and document the full `cargo clippy` command instead.

### Validation

```sh
cargo package --list --allow-dirty
scripts/check-package.sh
```

Inspect the package list and ensure it does not expose broken repo-only tooling as if it were usable from the published crate.

---

## 6. Safe facade pointer accessors

### Issue

Safe allocation wrappers expose `as_ptr(&self) -> *mut u8`.

Examples:

- `Allocation::as_ptr`
- `InitializedAllocation::as_ptr`
- `HeapAllocation::as_ptr`
- `InitializedHeapAllocation::as_ptr`

Returning a mutable raw pointer from `&self` is common in low-level code, but Rust-facing APIs often prefer const/mut separation to communicate aliasing intent more clearly.

### Best fix

For the safe facade, prefer:

```rust
pub const fn as_ptr(&self) -> *const u8;
pub const fn as_mut_ptr(&mut self) -> *mut u8;
```

For uninitialized allocation wrappers, also consider whether an explicit raw-mutable escape hatch should be named more clearly, such as `as_uninit_ptr` or `as_mut_ptr` requiring `&mut self`.

### Compatibility note

This is a public API change. Because the crate is still `0.1.0`, it can be fixed before stabilization. If compatibility with existing pre-release users matters, add the new methods first and deprecate the old `as_ptr -> *mut u8` shape before removing it.

### Validation

```sh
cargo test --features std safe::tests
cargo test --doc --features std
scripts/check-api-inventory.py
```

---

## 7. Odd naming: `try_clone_zeroed_contents`

### Issue

`InitializedAllocation::try_clone_zeroed_contents` allocates a new allocation and copies bytes from the existing allocation. The name suggests zeroing behavior, but the operation is really cloning initialized contents.

### Best fix

Rename to one of:

- `try_clone_contents`
- `try_clone_allocation`
- `try_clone_initialized`

Preferred: `try_clone_contents`.

If a `Clone` impl is intentionally avoided because allocation can fail, document that and keep the fallible method.

### Validation

```sh
cargo test --features std safe::tests
cargo test --doc --features std
```

---

## 8. Infallible APIs returning `Result`

### Issue

`Options::get(key: OptionKey) -> Result<c_long, OptionError>` cannot currently fail because `OptionKey` is already validated and closed over supported keys.

The callback guard constructors also currently return `Result` without a real failure path.

### Best fix

Make infallible APIs infallible.

Recommended changes:

- `Options::get(key: OptionKey) -> c_long`
- callback guard constructors return `Self` after reclassification, unless a real registration failure path is added.

Keep fallible APIs where failure is real:

- `OptionKey::from_raw(raw) -> Result<Self, OptionError>`
- `Options::set` if immutable/unsupported keys remain possible through the public type.

### Validation

```sh
cargo test --features std safe::tests
scripts/check-api-inventory.py
```

---

## 9. Public enums should be reviewed for future growth

### Issue

Public safe facade enums are currently exhaustive. Adding variants later is breaking unless the crate is still pre-stabilization.

Candidates:

- `TryAllocError`
- `ResizeErrorKind`
- `HeapError`
- `OptionError`
- `StatsError`
- `CallbackError` if retained
- `OptionKey`

### Best fix

Before stabilization, decide which enums are truly closed.

Recommended:

- Keep `OptionKey` exhaustive only if it deliberately models the small supported safe option set.
- Mark error enums `#[non_exhaustive]` if more errors are plausible.
- Remove unused variants before release rather than preserving speculative ones.

### Validation

```sh
cargo test --features std
cargo test --doc --features std
scripts/check-api-inventory.py
```

---

## 10. Stats and process-info type polish

### Issue

`StatCount` wraps `i64`, so negative counts are constructible through `StatCount::new`. Process time accessors return raw millisecond `usize` values.

This is not necessarily wrong because mimalloc stats can use signed counters internally, but it is not obvious at the safe API boundary.

### Best fix

Make the domain explicit.

Recommended:

- If public counts are never meant to be negative, use `u64`/`usize` or a validating constructor.
- If signed values are meaningful, rename or document `StatCount` as a signed statistic delta/count.
- Consider `core::time::Duration` for elapsed/user/system time accessors, or add duration-returning methods while keeping raw millisecond accessors if C parity is useful.

### Validation

```sh
cargo test --features std safe::tests
cargo test --doc --features std
```

---

## 11. Safe facade trait polish

### Issue

The initialized allocation wrappers expose byte slices, but do not implement standard slice conversion traits. Rust users may expect these wrappers to work with APIs accepting `AsRef<[u8]>` and `AsMut<[u8]>`.

### Best fix

Add standard trait impls where semantics are unambiguous:

- `AsRef<[u8]>` for initialized allocation wrappers.
- `AsMut<[u8]>` for initialized allocation wrappers.
- `TryFrom<i32>` for `OptionKey`.
- `std::error::Error::source` where wrapper errors contain another error.

Do not add `Deref` unless the allocation wrapper should behave like a slice in every context. `AsRef`/`AsMut` is lower commitment and clearer.

### Validation

```sh
cargo test --features std safe::tests
cargo test --doc --features std
```

---

## 12. Broad Clippy and dead-code allowances

### Issue

`Cargo.toml` contains many crate-level Clippy allowances. Several translation modules also retain `dead_code` and C naming allowances.

Some of this is appropriate for a mechanical C translation, but broad allowances make the crate feel less idiomatic to Rust maintainers.

### Best fix

Keep allowances where C ABI/parity requires them, but continue moving style allowances out of the safe facade and into the narrowest translation modules/items.

Recommended policy:

- Safe facade modules should remain free of broad dead-code and style allowances.
- New Rust-facing code should not rely on package-level mechanical-translation allowances.
- Any new allowance should be local and include a reason.
- Keep tests that assert the allowance inventory does not grow accidentally.

### Validation

```sh
cargo clippy --workspace --all-targets --all-features
scripts/check-lints.sh
cargo test --test release_fast_path_audit --features unstable-raw-internals
```

---

## 13. Toolchain and CI consistency

### Issue

The crate declares/pins Rust 1.92, but CI uses `dtolnay/rust-toolchain@stable`. That may not test the exact declared MSRV/toolchain. Platform docs say Linux x86_64 GNU is the maintained target, while CI also has Linux aarch64, macOS, and Windows jobs.

### Best fix

Make CI and platform docs say the same thing.

Recommended:

- Use the pinned toolchain or explicitly test MSRV in CI.
- If macOS/Windows/aarch64 are smoke-tested but not maintained targets, label them as smoke/non-tier jobs in workflow names and docs.
- If they are intended maintained targets, update `docs/platforms.md` with tier expectations and required evidence.

### Validation

Review:

- `.github/workflows/validation.yml`
- `rust-toolchain.toml`
- `Cargo.toml` `rust-version`
- `docs/platforms.md`
- `docs/reproducibility.md`

---

## 14. Build script portability

### Issue

`build.rs` manually invokes `cc`, `c++`, and `ar`. This can work, but Rust users often expect the `cc` crate to handle common platform/compiler details and cross-compilation conventions.

### Best fix

Either adopt the `cc` crate for C/C++ shim builds or document the current manual tool requirements as an intentional policy.

Recommended:

- For easier Rust ecosystem integration, use the `cc` crate unless manual command construction is required for precise ABI/link behavior.
- If keeping manual commands, keep `scripts/check-build-tools.sh` and expand cross-compilation docs.

### Validation

```sh
scripts/check-build-tools.sh
cargo check --features track-valgrind
cargo check --features track-asan
```

---

## 15. Repository hygiene

### Issue

`.gitignore` currently ignores only `/target`. The working tree can easily accumulate large or accidental untracked files such as benchmark scratch data, Redis dumps, local patches, generated CSVs, and temporary scripts.

### Best fix

Expand `.gitignore` for known local/generated artifacts while avoiding broad ignores that hide source files.

Recommended additions to consider:

- Python caches: `__pycache__/`, `*.pyc`
- local dumps: `dump.rdb`
- local benchmark scratch directories/files if generated
- editor/OS temporary files
- generated reports only if they are not intentionally versioned

Do not ignore files that are part of release evidence or source-controlled benchmark reports.

### Validation

```sh
git status --short
cargo package --list --allow-dirty
```

---

## Completion criteria

This polish pass is complete when:

1. all audit scripts pass and are required by CI/release checks;
2. unsafe borrowed callback guards live under `unsafe_callbacks`, are no longer classified as safe Tier S API, and docs plus API-inventory tooling enforce that classification;
3. Tier S rustdoc is complete and stale docs/examples are fixed;
4. `no_std`, package contents, and toolchain policy are clear and validated;
5. safe facade API names and ownership/borrowing shapes feel idiomatic;
6. broad translation allowances remain confined to C-shaped internals;
7. the mandatory Rust verification passes:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
```

Additional recommended gates:

```sh
scripts/check-api-inventory.py
scripts/check-safe-boundary.py
scripts/check-raw-facade.sh
scripts/check-ffi-boundary.py
scripts/check-layout.sh
scripts/check-abi-symbols.sh
scripts/check-features.sh
scripts/check-package.sh
scripts/check-docs.sh
scripts/check-unsafe-audit.py
scripts/check-atomic-ordering-audit.py
scripts/check-pointer-provenance-audit.py
scripts/check-no-allocation-audit.py
```
