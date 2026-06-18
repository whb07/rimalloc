# TODO: Safe Rust API and FFI/raw Boundary to Exemplary State

> Scope: This checklist is only for the public safe Rust API and the FFI/raw split of `mechanical_mimalloc`. It intentionally does not cover allocator-core parity work except where raw/FFI or safe API design depends on it.
>
> Goal: Once every item in this file is complete, the crate should have an exemplary public API boundary: a small, documented, semver-classified safe Rust facade; a clearly separated mimalloc-compatible C ABI; a deliberately unsafe raw Rust facade that does not leak accidental internals; and verification that prevents the old mechanical-port module graph from becoming public API again.

## Status legend

- `[ ]` Not started or not complete.
- `[~]` Partially implemented; needs follow-up before it can be treated as done.
- `[x]` Done and validated by the listed evidence.

A checkbox is not complete until its validation notes pass and the relevant documentation is updated.

## Non-goals

- Do not change allocator semantics unless the safe API or FFI boundary cannot be made sound without doing so.
- Do not broaden platform support claims. Linux x86_64 GNU remains the maintained target until the platform matrix has evidence.
- Do not stabilize a richer safe API merely to mirror the C API. Safe Rust wrappers should exist only when the crate can enforce their safety invariants internally.
- Do not expose allocator internals through `#[doc(hidden)]` as a substitute for real visibility boundaries.
- Do not remove C ABI compatibility symbols merely because they are inconvenient for Rust API design. The C ABI compatibility story is a separate compatibility domain.

## Global completion criteria

The safe Rust API / FFI split work is complete only when all of these are true:

- [ ] The crate root exposes only the safe Rust facade and the intentionally unsafe raw/FFI entrypoint module.
  - Validation:
    - `cargo test --test release_fast_path_audit crate_root_exposes_only_curated_rust_facade` passes.
    - `scripts/check-api-inventory.py` passes.
    - `cargo doc --no-deps --features std` shows no implementation module at crate root.
    - The public API inventory lists every crate-root item and classifies it as safe Rust API, raw facade, C ABI, or internal.

- [ ] `raw` no longer re-exports the entire mechanical-port module graph by default.
  - Validation:
    - There is an explicit allowlist for default `raw` exports.
    - `raw::internal`, `raw::page`, `raw::page_map`, `raw::theap`, `raw::arena`, `raw::os`, `raw::prim`, `raw::threadlocal`, and similar internal implementation modules are either absent, gated behind an explicit unstable feature, or classified in docs with a compatibility reason.
    - A source-shape test fails if a new implementation module is re-exported from `raw` without being added to the allowlist and docs.

- [ ] All C ABI declarations, raw C layouts, callback types, constants, and exported C symbols have a single documented ownership location.
  - Validation:
    - `src/abi.rs` or `src/raw/abi.rs` owns C header declarations and `repr(C)` layout definitions.
    - FFI/export wrappers live in one or more `ffi`/`c_api`/`exports` modules, not scattered arbitrarily across implementation modules without classification.
    - `tests/release_fast_path_audit.rs` or a dedicated script enforces that `#[unsafe(no_mangle)]`, `#[export_name]`, naked ABI shims, and global assembly exports are only in approved boundary files.

- [ ] Safe Rust APIs expose no raw allocator metadata pointers or lifetime-sensitive C handles unless the API is explicitly unsafe.
  - Validation:
    - `cargo doc --no-deps --features std` shows no public safe function taking or returning `*mut mi_heap_t`, `*mut mi_page_t`, `*mut mi_theap_t`, `*mut mi_arena_t`, `*mut mi_block_t`, or equivalent internal metadata pointers.
    - A compile-fail test proves that `core_types` and implementation modules are inaccessible to downstream users.
    - Any public raw-pointer function outside `raw` is marked `unsafe` and has a `# Safety` section.

- [ ] Safe Rust APIs have full rustdoc, examples, error behavior, panic behavior, and feature notes.
  - Validation:
    - `cargo test --doc --features std` passes.
    - `cargo doc --no-deps --features std` passes with `rustdoc::broken_intra_doc_links = deny`.
    - Every public safe type has examples or a deliberate “no example because...” note.
    - Every public unsafe item has a `# Safety` section.

- [ ] The FFI/raw split preserves the dynamic C ABI surface.
  - Validation:
    - `scripts/check-abi-symbols.sh` passes.
    - `scripts/check-layout.sh` passes.
    - C smoke examples compile and link against the produced artifact.
    - Symbol diffs show no removed mimalloc-compatible symbol unless the change is explicitly documented as a breaking ABI change.

- [ ] The safe API and raw API compatibility domains are documented and enforced.
  - Validation:
    - `docs/api-inventory.md`, `docs/compatibility.md`, `docs/abi.md`, and `CHANGELOG.md` are updated.
    - Safe API changes are classified as Rust SemVer surface.
    - `raw` facade changes are classified as unstable raw Rust surface unless specifically stabilized.
    - C ABI/layout changes are classified independently from Rust SemVer.

- [ ] The local signoff command set is updated for this workstream.
  - Validation:
    - `scripts/check-local.sh` includes API inventory, docs, features, ABI/layout, and examples.
    - A new or existing raw-facade check prevents accidental re-export creep.
    - Release policy names the safe API / FFI split checks explicitly.

---

# Workstream A: Public surface contract

## A1. Define the crate identity in one authoritative place

- [x] State the crate identity in `README.md`, crate docs, and `docs/compatibility.md` using the same wording.
  - Required wording decision:
    - The crate is a mimalloc-compatible C ABI allocator artifact.
    - The crate also provides a narrow, experimental safe Rust allocation facade.
    - The crate is not yet a broad safe Rust heap/options/stats abstraction unless those wrappers are completed below.
  - Validation:
    - `README.md`, `src/lib.rs` crate docs, and `docs/compatibility.md` agree.
    - `docs/api-inventory.md` lists the same public surface as rustdoc.
    - No documentation claims stabilized safe APIs that do not exist.
  - Evidence:
    - `README.md`, `src/lib.rs`, and `docs/compatibility.md` use the shared identity statement: mimalloc-compatible C ABI allocator artifact, narrow experimental safe Rust allocation facade, not a broad safe heap/options/stats abstraction.
    - `scripts/check-api-inventory.py` passes and inventories the crate-root public surface.

- [x] Define public API tiers.
  - Required tiers:
    - Tier S: safe Rust API, semver surface.
    - Tier R: raw Rust facade, unsafe and unstable unless explicitly versioned.
    - Tier C: exported C ABI and raw layout compatibility surface.
    - Tier I: internal implementation surface, not public API.
  - Validation:
    - `docs/compatibility.md` contains these tiers.
    - `docs/api-inventory.md` assigns every public item to exactly one tier.
    - A check script fails when a new public crate-root item is not inventoried.
  - Evidence:
    - `docs/compatibility.md` and `docs/api-inventory.md` define Tier S, Tier R, Tier C, and Tier I.
    - `scripts/check-api-inventory.py` validates crate-root inventory drift and required tier policy wording.

- [x] Define the release blocker rule for accidental public API.
  - Required policy:
    - Any new `pub` item outside the crate-root facade or `raw` allowlist is release-blocking until classified.
    - Any new `#[unsafe(no_mangle)]` or `#[export_name]` outside approved FFI/export files is release-blocking.
  - Validation:
    - Source-shape audit enforces the rule.
    - The release checklist points to the audit command.
  - Evidence:
    - `docs/compatibility.md` documents new unclassified `pub` items and export attributes outside approved boundary files as release-blocking.
    - `scripts/check-api-inventory.py` enforces crate-root public item inventory.
    - `scripts/check-ffi-boundary.py` enforces approved files for `no_mangle`, `export_name`, naked ABI shims, and `global_asm!`/`naked_asm!`.
    - `docs/release.md` and `scripts/check-local.sh` run the API inventory and FFI boundary checks.

## A2. Stabilize the crate-root facade shape

- [x] Keep crate-root public items deliberately small.
  - Intended default root surface:
    - `pub mod raw;`
    - `pub struct MiMalloc;`
    - `pub struct Allocation` or its final replacement type(s).
    - `pub struct AllocError`.
    - `pub struct LayoutError`.
    - `pub enum TryAllocError`.
    - Any future safe `Heap`, `StatsSnapshot`, `Options`, `ProcessInfo`, or `Collect` API only after the corresponding workstream is done.
  - Validation:
    - `scripts/check-api-inventory.py` passes.
    - `cargo doc --no-deps --features std` shows only the curated items at crate root.
    - Compile-fail docs prove old modules such as `mechanical_mimalloc::alloc` and `mechanical_mimalloc::page` are unavailable.
  - Evidence:
    - `src/lib.rs` exposes `raw`, `MiMalloc`, `Allocation`, `AllocError`, `LayoutError`, and `TryAllocError` only.
    - `scripts/check-api-inventory.py` validates that exact crate-root allowlist.
    - Crate docs include compile-fail examples for inaccessible implementation modules.

- [x] Do not export internal modules from crate root.
  - Modules that must remain non-public at crate root:
    - `abi`, `alloc`, `alloc_aligned`, `alloc_override`, `alloc_posix`, `arena`, `arena_meta`, `atomic`, `bitmap`, `bits`, `core_types`, `free`, `headers`, `heap`, `init`, `internal`, `libc`, `memory`, `mimalloc`, `options`, `os`, `page`, `page_map`, `prim`, `random`, `static_c`, `stats`, `theap`, `threadlocal`, `track`, `types`.
  - Validation:
    - Release audit searches `src/lib.rs` for `pub mod` declarations for these names and fails if found.
    - Compile-fail tests cover at least five representative old paths.
  - Evidence:
    - `cargo test --quiet --test release_fast_path_audit crate_root_exposes_only_curated_rust_facade` passes.
    - Crate docs include compile-fail coverage for `alloc`, `page`, `heap`, `internal`, `options`, and `os` paths.

- [x] Decide whether `raw` is documented at crate root as unstable, unsafe, or both.
  - Required policy:
    - `raw` is unsafe because most operations inherit C pointer/lifetime/threading requirements.
    - `raw` is unstable as a Rust module path surface until explicitly versioned.
    - C ABI symbol compatibility is still tracked separately and may be stronger than `raw` Rust-path compatibility.
  - Validation:
    - `src/lib.rs` docs and `docs/api-inventory.md` both say this.
    - `raw` module docs include a warning before listing re-exports.
  - Evidence:
    - `src/lib.rs` documents `raw` as unsafe because callers must uphold C pointer ownership, layout, alignment, threading, and lifetime requirements.
    - `docs/api-inventory.md` documents `raw` as Tier R and unstable until explicitly versioned.
    - `src/raw.rs` documents that raw module paths are unstable and that C ABI symbol compatibility is tracked separately.

## A3. Public documentation gates

- [x] Add `#![warn(missing_docs)]` or a documented staged alternative once root surface is stable.
  - Validation:
    - If enabled: `cargo doc --no-deps --features std` passes without public missing-doc warnings for the safe facade.
    - If staged: `docs/release.md` lists the blockers preventing `missing_docs = warn/deny`.
  - Evidence:
    - `docs/release.md` documents the staged `missing_docs` alternative and blockers: narrow/gate raw internals, complete Tier S examples/error notes, add unsafe `# Safety` docs or gate Tier R internals, then run `scripts/check-docs.sh`.

- [x] Add docs.rs behavior that matches the public story.
  - Required behavior:
    - Docs build with `std` for examples and error traits.
    - Bench-only APIs are not presented as normal library APIs.
    - Raw internals are hidden or explicitly unstable.
  - Validation:
    - `[package.metadata.docs.rs]` is correct.
    - `cargo doc --no-deps --features std` matches docs.rs expectations.
    - `docs/api-inventory.md` notes any docs.rs-only cfg.
  - Evidence:
    - `Cargo.toml` sets docs.rs `features = ["std"]` and `rustdoc-args = ["--cfg", "docsrs"]`.
    - `docs/api-inventory.md` states that there are no docs.rs-only public APIs and benchmark targets are not normal rustdoc APIs.
    - `scripts/check-docs.sh` passes.

---

# Workstream B: Raw and FFI module architecture

## B1. Define the final module taxonomy

- [ ] Split raw/FFI responsibilities into named layers.
  - Target taxonomy:
    - `raw::abi`: `repr(C)` layouts, C-compatible aliases, constants, callback type aliases, and extern declarations that mirror public C headers.
    - `raw::c_api` or `raw::mimalloc`: direct unsafe C-shaped functions intended for low-level Rust callers.
    - `ffi` or `c_api_exports`: Rust implementations of exported C ABI symbols and symbol aliases.
    - `compat` or `internal_compat`: translated C-shaped helpers needed internally but not public Rust API.
    - allocator internals: private Rust modules that own implementation logic.
  - Validation:
    - A module diagram in `docs/abi.md` or `docs/architecture.md` matches the actual tree.
    - `raw.rs` no longer acts as an unstructured re-export bucket.
    - `scripts/check-api-inventory.py` enforces the taxonomy.

- [ ] Decide final file layout.
  - Acceptable layout examples:
    - `src/raw.rs` plus `src/raw/abi.rs`, `src/raw/c_api.rs`, `src/raw/types.rs`.
    - `src/ffi.rs` plus `src/ffi/exports.rs`, `src/ffi/callbacks.rs`, `src/ffi/layout.rs`.
    - Or a documented equivalent.
  - Validation:
    - File layout is documented in `docs/abi.md`.
    - There is no ambiguity about where to add a new C ABI declaration or exported function.

- [x] Move raw C header declarations into one owner.
  - Required owner:
    - Public `mi_*` function declarations.
    - Public C constants.
    - Public callback type aliases.
    - Public C layout aliases.
  - Validation:
    - `src/abi.rs` or replacement owner contains the declarations.
    - `src/mimalloc.rs` does not contain its own `unsafe extern "C"` public header mirror.
    - Audit test verifies this.
  - Evidence:
    - `src/abi.rs` owns public C header declarations, constants, callback aliases, and layout aliases.
    - `src/mimalloc.rs` is an internal compatibility alias and contains no `unsafe extern "C"` header mirror.
    - `scripts/check-api-inventory.py` verifies the ownership split and requires the `docs/abi.md` module taxonomy section.

## B2. Narrow the default `raw` facade

- [x] Create a default `raw` export allowlist.
  - Recommended default allowlist:
    - `raw::abi` or `raw::mimalloc` for public C API declarations.
    - Public C-compatible types needed to call those APIs.
    - Public C constants and callback type aliases.
    - Maybe `raw::stats`, `raw::options`, `raw::heap` only if they contain public C-header surface, not internal helpers.
  - Validation:
    - A script prints the default `raw` public modules and compares them to an allowlist file.
    - `docs/api-inventory.md` lists the default raw modules.
    - No raw default module exists only because a test/bench previously used an implementation path.
  - Evidence:
    - `src/raw.rs` exposes only `raw::abi`, `raw::headers`, `raw::mimalloc`, and root `raw::mi_*` C-header re-exports by default.
    - `scripts/check-api-inventory.py` enforces the default raw module allowlist.
    - `scripts/check-raw-facade.sh` enforces no implementation-module `pub use crate::<module>::*` leaks into default raw.
    - `docs/api-inventory.md` lists the default raw modules.

- [x] Move internal raw re-exports behind `unstable-raw-internals` or remove them.
  - Candidate modules to remove or gate by default:
    - `raw::internal`
    - `raw::page`
    - `raw::page_map`
    - `raw::theap`
    - `raw::arena`
    - `raw::arena_meta`
    - `raw::os`
    - `raw::prim`
    - `raw::threadlocal`
    - `raw::random`
    - `raw::memory`
    - `raw::bitmap`
    - `raw::bits`
    - `raw::headers`
    - `raw::static_c`
    - `raw::track`, unless it is a public tracking API surface.
  - Validation:
    - Default rustdoc does not show these modules.
    - Existing tests/benches are migrated to crate-internal paths or an explicit unstable feature.
    - A downstream compile-fail test verifies `mechanical_mimalloc::raw::internal` is unavailable by default.
  - Evidence:
    - Implementation modules in `src/raw.rs` are gated behind `#[cfg(feature = "unstable-raw-internals")]`.
    - Integration tests that still inspect raw internals are gated with `#![cfg(feature = "unstable-raw-internals")]`.
    - `cargo test --quiet --features std` passes with default raw internals unavailable.
    - `cargo test --quiet --features "std unstable-raw-internals"` passes and covers the gated internal audits.

- [x] If an unstable internals feature is kept, make it unmistakably unstable.
  - Required behavior:
    - Feature name includes `unstable`, for example `unstable-raw-internals`.
    - Feature is not enabled by default, not enabled by `std`, and not enabled by `bench` unless absolutely necessary.
    - Docs state it has no semver guarantees.
  - Validation:
    - Feature matrix documents it.
    - `cargo check` default does not expose the internals.
    - `cargo check --features unstable-raw-internals` exposes only the allowlisted unstable modules.
    - `scripts/check-features.sh` covers it if present.
  - Evidence:
    - `Cargo.toml` defines `unstable-raw-internals`, leaves it out of `default`, `std`, and `bench`, and documents that it is not public API.
    - `docs/api-inventory.md`, `docs/compatibility.md`, and `docs/raw-facade-migration.md` state that the feature is off by default and has no SemVer guarantees.
    - `scripts/check-features.sh` checks `cargo check --features unstable-raw-internals` and `cargo test --quiet --features "std unstable-raw-internals"`.

- [ ] Remove test/benchmark dependence on public raw internals where possible.
  - Required migration:
    - Integration tests that need internals should use public C API behavior or crate-private unit tests.
    - Benchmarks should call public C ABI or safe Rust facade, not private raw internals.
  - Validation:
    - `grep -R "mechanical_mimalloc::raw::internal\|mechanical_mimalloc::raw::page\|mechanical_mimalloc::raw::theap" tests benches examples` returns no default-feature uses unless intentionally allowlisted.
    - If any remain, they are behind `unstable-raw-internals` and documented.

## B3. Separate exported C ABI from internal Rust calls

- [ ] Create explicit exported-function wrapper files.
  - Required behavior:
    - `#[unsafe(no_mangle)]`, `#[export_name]`, naked exported functions, and global assembly exported public aliases live in approved boundary files or are clearly classified if assembly must remain near hot paths.
    - Boundary wrappers validate/convert inputs where possible, call internal Rust functions, and convert results back to C null/errno conventions.
  - Validation:
    - A script lists every `no_mangle`, `export_name`, naked exported function, and global public symbol alias and compares paths against an allowlist.
    - `docs/abi.md` describes why any exception remains near hot-path assembly.

- [ ] Stop internal Rust code from re-entering exported C ABI functions except when preserving C semantics requires it.
  - Required behavior:
    - Internal code calls crate-private implementation functions, not raw public extern declarations.
    - Re-entry through C ABI is classified as compatibility-required, benchmark-required, or removed.
  - Validation:
    - Source audit searches for calls from internals to `raw::mi_*` or `abi::mi_*` and fails unless allowlisted.
    - Each allowlisted re-entry has a comment explaining why internal direct calls are not correct.

- [ ] Create typed internal request/result conversion at the FFI boundary.
  - Required conversions:
    - `usize` size/count/alignment inputs to `LayoutRequest`, `CallocRequest`, `ReallocRequest`, `Alignment`, `RequestedSize`, etc.
    - Raw heap/page pointers to crate-private checked wrappers where validation is possible.
    - Internal errors to C null/errno/abort conventions.
  - Validation:
    - Representative public allocation entry points construct typed request objects before allocator-core dispatch.
    - Audit tests verify allocation/free/realloc entry points cross a typed validation boundary.
    - C parity tests still pass.

- [ ] Preserve C ABI names while using Rust-native internal names.
  - Required naming split:
    - `mi_*` names remain in C ABI/raw layers.
    - Internal helpers use Rust names or typed methods where ownership/invariants are clearer.
    - New internal `mi_*` functions are allowed only for direct compatibility shims.
  - Validation:
    - New internal APIs added by this workstream are reviewed for naming.
    - `docs/architecture.md` or equivalent explains naming rules.

## B4. Raw layout and ABI representation

- [ ] Keep all C-compatible structs under raw/ABI ownership.
  - Required behavior:
    - Public C layout types are `#[repr(C)]` or documented `#[repr(transparent)]` wrappers.
    - Field order, size, alignment, and offsets are asserted.
    - Feature-dependent layout is documented.
  - Validation:
    - `scripts/check-layout.sh` passes for default and relevant feature sets.
    - Layout assertions cover every public C layout used through `raw`.
    - `docs/abi.md` lists layout-sensitive feature flags.

- [ ] Avoid Rust enums for C-extensible integer domains unless the domain is closed.
  - Required behavior:
    - C enums or integer domains that may receive unknown values use transparent integer newtypes.
    - Conversion methods validate known values without making unknown values UB.
  - Validation:
    - Audit checks all public raw enum-like types.
    - Tests cover unknown/raw values where the C ABI may pass them.

- [ ] Define raw pointer nullability in docs and types.
  - Required behavior:
    - Raw C APIs use raw pointers where null is valid or inherited from C.
    - Unsafe Rust convenience wrappers may use `NonNull` only when null is impossible or checked.
    - Safe APIs do not expose nullable raw pointers.
  - Validation:
    - Public unsafe docs include nullability for every raw pointer parameter.
    - Tests cover null-accepted APIs such as free and null realloc behavior.

## B5. Callback boundary

- [ ] Inventory all C callback types and registrations.
  - Callback areas:
    - output callback
    - error callback
    - deferred free callback
    - heap/block visit callbacks
    - commit/decommit/purge/platform callback types, if public
  - Validation:
    - `docs/abi.md` has a callback ABI section listing every callback family.
    - Each callback type has docs for nullability, lifetime, threading, reentrancy, and unwind behavior.

- [ ] Keep callback registration raw/unsafe unless a sound RAII wrapper is designed.
  - Required behavior:
    - Safe API must not let users register a callback with dangling context.
    - Safe callback guards must unregister or restore previous callback on drop, or document why that cannot be guaranteed.
    - Callback closures must not allocate through the same allocator unless reentrancy behavior is documented.
  - Validation:
    - No safe callback registration API exists unless a soundness proof is in `docs/unsafe-audit.md`.
    - If implemented, tests cover registration, callback invocation, guard drop, panic prevention, and reentrancy notes.

- [ ] Prevent Rust panics from crossing callback/FFI boundaries.
  - Required behavior:
    - Raw docs state callbacks must not unwind.
    - If safe callback wrappers exist, they catch/abort or otherwise prevent unwinding across C.
  - Validation:
    - `tests/ffi_panic_audit.rs` covers exported boundary unwind policy.
    - Documentation includes panic behavior.

---

# Workstream C: Safe Rust API foundation

## C1. Safe API design principles

- [x] Document the safe API design rules.
  - Required rules:
    - Safe APIs must not require callers to uphold pointer provenance, heap ownership, aliasing, or thread-local invariants.
    - Raw C null/error conventions are translated into Rust `Result`, `Option`, or safe no-op behavior.
    - Safe APIs must not panic except for documented programmer errors.
    - Safe APIs must not allocate internally in allocator hot paths or error paths unless explicitly outside allocator operation.
    - Safe wrappers around handles use RAII only when RAII matches mimalloc semantics.
  - Validation:
    - `docs/api-inventory.md` or `docs/safe-api.md` includes these rules.
    - Every safe public item has a short statement of what invariants it enforces.
  - Evidence:
    - `docs/safe-api.md` documents the safe API rules for pointer provenance, heap ownership, aliasing, thread-local invariants, C null/error translation, panic behavior, hot-path/error-path allocation, and RAII handle semantics.
    - `docs/api-inventory.md` and rustdoc describe the current safe public items and their enforced invariants.

- [x] Define safe API naming conventions.
  - Required conventions:
    - Rust-facing names are idiomatic: `MiMalloc`, `Allocation`, `Heap`, `StatsSnapshot`, `Options`.
    - C names remain under `raw`/`ffi`.
    - Fallible methods use `try_` when there is also an infallible variant or when Rust convention benefits.
    - Unsafe constructors use `from_raw`, `from_raw_unchecked`, `as_raw`, `into_raw` only with explicit safety docs.
  - Validation:
    - Public API inventory passes a naming review.
    - Rustdoc examples use Rust names unless demonstrating raw C API.
  - Evidence:
    - `docs/safe-api.md` defines Rust-facing names, C-name placement under `raw`/C ABI, fallible `try_` naming, and raw ownership-transfer naming.
    - `scripts/check-api-inventory.py` passes and rustdoc examples use Rust names except in raw examples.

- [x] Define safe API trait policy.
  - Required behavior:
    - Implement `Debug`, `Default`, `Copy`, `Clone`, `Eq`, `Hash`, `Send`, and `Sync` only where semantically correct.
    - Every unsafe `Send` or `Sync` impl has a proof in rustdoc and unsafe audit docs.
    - Do not implement `Deref` for handle wrappers unless deref target semantics are obvious and safe.
  - Validation:
    - Trait impls are listed in `docs/api-inventory.md`.
    - Unsafe trait impls have `SAFETY:` comments and audit sections.
  - Evidence:
    - `docs/safe-api.md` defines trait policy for `Debug`, `Default`, `Copy`, `Clone`, `Eq`, `Hash`, `Send`, `Sync`, and `Deref`.
    - `docs/api-inventory.md` lists current safe API trait impls.
    - No current safe API type has a manual unsafe `Send` or `Sync` impl.

## C2. `MiMalloc` global allocator facade

- [x] Finalize `MiMalloc` semantics.
  - Required docs:
    - `GlobalAlloc` implementation scope.
    - Zero-size layout behavior and Rust `GlobalAlloc` contract.
    - Alignment behavior and when aligned allocation path is used.
    - Allocation failure behavior.
    - Deallocation layout expectations.
    - Reallocation behavior and preservation guarantee inherited from allocator.
    - Panic/unwind behavior.
    - Thread behavior.
    - Feature interactions with `secure`, `guarded`, tracking, and `std`.
  - Validation:
    - `MiMalloc` rustdoc contains these sections.
    - `examples/global_allocator.rs` compiles.
    - `cargo test --doc --features std` passes.
  - Evidence:
    - `src/lib.rs` rustdoc for `MiMalloc` documents `GlobalAlloc` scope, zero-size layout behavior, alignment dispatch, allocation failure, deallocation layout expectations, realloc preservation/failure behavior, panic/unwind behavior, thread behavior, and feature interactions.
    - `cargo test --quiet --example global_allocator --features std` passes.
    - `cargo test --doc --features std` passes.

- [x] Audit `MiMalloc::alloc` alignment dispatch.
  - Required behavior:
    - Alignment threshold uses the allocator's actual maximum default alignment rule.
    - Over-aligned layouts call aligned allocation.
    - Invalid layouts are impossible because `GlobalAlloc` receives `Layout`.
  - Validation:
    - Unit/integration tests allocate through `MiMalloc` for alignments 1, 2, 4, 8, 16, 32, 64, page alignment, and a large power-of-two alignment.
    - Tests verify pointer alignment and free without corruption.
    - Secure/guarded feature tests include at least one over-aligned case.
  - Evidence:
    - `tests/mimalloc_global_alloc.rs` covers alignments 1, 2, 4, 8, 16, 32, 64, 4096, and 1 MiB through `MiMalloc`.
    - The test verifies returned pointer alignment and frees every allocation.
    - `cargo test --quiet --test mimalloc_global_alloc`, `cargo test --quiet --test mimalloc_global_alloc --features secure`, and `cargo test --quiet --test mimalloc_global_alloc --features guarded` pass.

- [x] Audit `MiMalloc::realloc` alignment behavior.
  - Required behavior:
    - Reallocating over-aligned allocations preserves alignment requirements.
    - Realloc failure behavior matches `GlobalAlloc` expectations.
    - Old contents are preserved up to `min(old_size, new_size)`.
  - Validation:
    - Tests cover grow and shrink for normal and over-aligned layouts.
    - Tests fill old allocation with a pattern and verify preservation.
    - Tests run under default, `secure`, and `guarded` where feasible.
  - Evidence:
    - `tests/mimalloc_global_alloc.rs` covers grow and shrink reallocations for normal, 64-byte, and page-aligned layouts.
    - The realloc test writes a deterministic byte pattern and verifies preservation through `min(old_size, new_size)`.
    - `cargo test --quiet --test mimalloc_global_alloc`, `cargo test --quiet --test mimalloc_global_alloc --features secure`, and `cargo test --quiet --test mimalloc_global_alloc --features guarded` pass.

- [x] Define whether `MiMalloc` is `Default`, `Debug`, `Copy`, `Clone`, `Send`, and `Sync`.
  - Recommended state:
    - `MiMalloc` is a zero-sized type.
    - `Default`, `Debug`, `Copy`, and `Clone` are acceptable if documented.
    - `Send`/`Sync` are auto traits if no fields; document thread-safety inherited from global allocator semantics.
  - Validation:
    - Compile tests cover expected trait bounds.
    - Rustdoc lists trait behavior.
  - Evidence:
    - `MiMalloc` derives `Default`, `Debug`, `Copy`, and `Clone`.
    - `tests/mimalloc_global_alloc.rs` compile-checks `MiMalloc: Copy + Clone + Default + Debug + Send + Sync`.
    - `src/lib.rs`, `docs/api-inventory.md`, and `docs/safe-api.md` document the trait behavior and lack of manual unsafe `Send`/`Sync` impls.

- [x] Ensure `MiMalloc` examples avoid misleading allocator recursion.
  - Required behavior:
    - Examples do not allocate during allocator initialization in ways that hide bugs.
    - Examples explain that `GlobalAlloc` affects process-wide Rust allocation after declaration.
  - Validation:
    - `examples/global_allocator.rs` and crate docs compile and run under `std`.
  - Evidence:
    - `examples/global_allocator.rs` declares a static `MiMalloc` global allocator and performs ordinary `Vec`/`Box` allocations in `main`, after allocator selection.
    - `src/lib.rs` documents that `#[global_allocator]` affects process-wide Rust allocation after declaration.
    - `cargo test --quiet --example global_allocator --features std` and `cargo test --doc --features std` pass.

## C3. Safe owned allocation wrapper

- [x] Decide final owned allocation type shape.
  - Design decision:
    - Keep one `Allocation` type with raw pointer access only.
    - Or split into `Allocation<Uninit>` and `Allocation<Init>` / `ZeroedAllocation` to avoid safe reads from uninitialized memory.
    - Or expose only zeroed initialized byte allocations safely and keep uninitialized allocation unsafe.
  - Validation:
    - The chosen design is documented in rustdoc.
    - There is no safe method that can read uninitialized bytes.
    - Tests cover zeroed allocation reads and uninitialized allocation write-before-read behavior.
  - Evidence:
    - `src/safe.rs` rustdoc documents the current one-type `Allocation` design with raw pointer access only.
    - `Allocation` exposes no safe byte-slice read API; rustdoc includes a compile-fail example for `allocation.as_bytes()`.
    - Unit tests cover zeroed allocation reads and write-before-read behavior for uninitialized allocation through raw pointers.

- [x] Audit `Allocation::new` for uninitialized memory soundness.
  - Required behavior:
    - Safe code must not be able to observe uninitialized bytes through safe APIs.
    - Any method returning `&[u8]`, `&mut [u8]`, or reading bytes exists only for initialized/zeroed states.
    - Raw pointer escape hatches document initialization obligations.
  - Validation:
    - Search public methods for slice/reference returns.
    - Compile tests prove uninitialized allocation cannot be read safely.
    - Miri/model tests cover initialized paths if applicable.
  - Evidence:
    - `Allocation::new` returns uninitialized memory but `Allocation` exposes only metadata and raw pointer escape hatches.
    - `rg -n "pub .*(&\\[|&mut \\[)|pub .*as_bytes|pub .*as_uninit" src/safe.rs` finds no public slice/reference byte accessors.
    - Rustdoc compile-fail coverage proves uninitialized allocations cannot be read through a safe `as_bytes` API.

- [x] Document raw pointer escape hatches.
  - Required docs for `as_ptr` / `as_non_null` / equivalents:
    - Pointer is valid only while the allocation is alive.
    - Caller must not free it manually.
    - Caller must not create aliasing references that conflict with Rust rules.
    - Caller must not read uninitialized memory.
    - Pointer may be dangling/sentinel for zero-sized allocations.
  - Validation:
    - Rustdoc for every escape hatch has these notes.
    - `cargo doc` and doctests pass.
  - Evidence:
    - `Allocation::as_non_null` and `Allocation::as_ptr` rustdoc state lifetime, no manual free, aliasing, no uninitialized reads, and zero-sized sentinel behavior.
    - `scripts/check-docs.sh` passes.

- [x] Define zero-sized allocation behavior.
  - Required behavior:
    - Zero-sized `Allocation` does not call the allocator.
    - Pointer value is aligned for the requested layout or uses `NonNull::dangling` strategy with equivalent alignment guarantees.
    - Drop is a no-op.
  - Validation:
    - Unit tests cover multiple alignments for size 0.
    - Tests assert no allocator free is attempted for size 0 if instrumentation is available.
    - Docs state behavior.
  - Evidence:
    - `Allocation` rustdoc states zero-sized allocations do not call the allocator, use an aligned sentinel pointer, and drop is a no-op.
    - Unit tests cover zero-sized allocations at alignments 1, 2, 4, 8, 16, 64, and 4096.
    - Drop guards on `layout.size() != 0`; no allocator-free instrumentation exists for this wrapper.

- [x] Define `Allocation` move, drop, and ownership semantics.
  - Required behavior:
    - `Allocation` is move-only, not `Copy` or `Clone` unless clone performs allocation and copy intentionally.
    - Drop never panics.
    - Drop frees exactly once.
    - `into_raw`/`from_raw` exist only if there is a sound ownership transfer story.
  - Validation:
    - Compile tests prove `Allocation: !Copy`.
    - Unit tests move allocation and verify single free through leak/double-free sanitization or debug instrumentation where feasible.
    - Unsafe `from_raw` docs list allocator-origin and layout requirements if implemented.
  - Evidence:
    - `Allocation` rustdoc states it is move-only, frees on drop, and has no `Clone`, `Copy`, `from_raw`, `into_raw`, or resize API.
    - Rustdoc compile-fail coverage proves use-after-move is rejected.
    - Unit tests move an allocation and then drop the moved owner; debug instrumentation for exact free counts is not currently available.

- [x] Add safe slice APIs only after initialization semantics are settled.
  - Candidate APIs:
    - `as_bytes(&self) -> &[u8]` only for initialized/zeroed allocation.
    - `as_bytes_mut(&mut self) -> &mut [u8]` only for initialized/zeroed allocation.
    - `as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>]` for uninitialized allocation.
  - Validation:
    - Miri or model tests confirm no uninitialized reads in safe code.
    - Rustdoc examples demonstrate correct initialization before reading.
  - Evidence:
    - No safe slice APIs are exposed yet because initialization typestate is not settled.
    - Rustdoc documents that future slice APIs require an initialization model that cannot expose uninitialized bytes.
    - Unit coverage demonstrates write-before-read through raw pointers; there is no safe initialized slice API requiring Miri/model coverage.

- [x] Add `try_resize` or deliberately omit resizing from `Allocation`.
  - Required decision:
    - If resize exists, it must preserve contents and update layout correctly.
    - If resize is omitted, docs explain users should allocate/copy manually or use raw API.
  - Validation:
    - Grow/shrink tests cover initialized data preservation and alignment.
    - Failure path leaves original allocation valid.
    - Error type includes both old and requested layout where useful without allocating.
  - Evidence:
    - `Allocation` deliberately omits resize APIs for now.
    - Rustdoc says users should allocate a new block and copy through raw pointers if resizing is needed before a safe resize API is designed.
    - No grow/shrink or resize-failure tests apply until a resize API exists.

- [x] Decide whether `Allocation` is `Send` and/or `Sync`.
  - Required reasoning:
    - `Send` may be sound if ownership of the allocation can move across threads and allocator free is cross-thread safe.
    - `Sync` is likely not meaningful unless shared references expose no mutation and data initialization semantics are safe.
    - Do not add unsafe impls without proof.
  - Validation:
    - Trait behavior is explicit in docs and tests.
    - Unsafe audit includes proof if manual impls are added.
  - Evidence:
    - `Allocation` has manual `Send` and `Sync` impls with `SAFETY:` comments.
    - `docs/api-inventory.md`, `docs/safe-api.md`, and `docs/unsafe-audit.md` document the trait behavior and proof.
    - Unit tests compile-check `Allocation: Send + Sync`.

## C4. Error and status types

- [x] Finalize error hierarchy.
  - Minimum current types:
    - `AllocError`
    - `LayoutError`
    - `TryAllocError`
  - Future safe API types as needed:
    - `HeapError`
    - `StatsError`
    - `OptionsError`
    - `CallbackError`
    - `UnsupportedError`
  - Validation:
    - Error docs explain cause and recovery.
    - `Display` does not allocate.
    - `std::error::Error` impls are gated by `std`.
    - Errors are `Copy`/`Clone` only when semantically stable and cheap.
  - Evidence:
    - `src/safe.rs` documents cause and recovery for `AllocError`, `LayoutError`, and `TryAllocError`.
    - Unit tests format errors through a fixed caller-provided `fmt::Write` buffer.
    - `std::error::Error` impls are gated with `#[cfg(feature = "std")]`.
    - Unit tests compile-check the safe error types as `Copy + Clone + Debug + Eq`.

- [x] Avoid stringly or ambiguous errors.
  - Required behavior:
    - No public safe API returns `bool` for failure unless the meaning is obvious and documented.
    - No public safe API returns raw errno integers without a typed wrapper.
    - No error path constructs `String` inside allocator-sensitive code.
  - Validation:
    - Public API review checks return types.
    - No `String`, `Vec`, `Box`, or formatting allocation occurs in allocator hot/error paths.
    - `tests/no_allocation_critical_paths.rs` or equivalent covers this.
  - Evidence:
    - Public safe allocation APIs return `Result<_, AllocError>` or `Result<_, TryAllocError>`; there are no safe bool/errno failure returns.
    - `rg -n "String|Vec|Box|format!|to_string" src/safe.rs` finds no owned allocation or string construction in safe error paths.
    - `tests/no_allocation_critical_paths.rs` covers allocator critical paths, and safe error `Display` tests use a fixed stack buffer.

- [x] Define mapping from raw C failure to Rust errors.
  - Required mapping examples:
    - Null from allocation => `AllocError`.
    - Invalid size/alignment => `LayoutError`.
    - Unsupported platform/feature => `UnsupportedError` if exposed safely.
    - Raw errno behavior stays under `raw`.
  - Validation:
    - Docs include mapping table.
    - Tests cover null/overflow allocation requests and invalid alignment.
  - Evidence:
    - `docs/safe-api.md` includes the raw-condition-to-safe-error mapping table.
    - `Allocation::allocate` maps null from `MiMalloc` to `AllocError`.
    - Unit tests cover invalid alignment and layout overflow mapping to `TryAllocError::Layout`.

---

# Workstream D: Optional safe `Heap` API

Safe heap wrappers are not required for the minimal safe Rust facade, but if the crate wants exemplary allocator-library status, this workstream must be completed or explicitly deferred.

## D1. Decide heap API scope

- [x] Decide whether to stabilize any safe heap API in the next release milestone.
  - Options:
    - No safe heap API; raw heap API only.
    - Minimal owned `Heap` with allocation methods and RAII deletion.
    - Full heap/options/stats/subprocess wrapper family.
  - Validation:
    - Decision is recorded in `docs/api-inventory.md` and README.
    - If deferred, docs say safe heap API is intentionally absent, not forgotten.
  - Evidence:
    - `docs/api-inventory.md`, `docs/safe-api.md`, and README state that safe heap wrappers are intentionally deferred.
    - Raw heap functions remain available only under `raw`.

- [x] Define heap ownership model.
  - Required concepts if implemented:
    - `Heap`: owned heap created by `mi_heap_new` and deleted on drop.
    - `MainHeap` or `HeapRef`: borrowed handle for `mi_heap_main` that must not be deleted.
    - `HeapAllocation<'heap>` or equivalent to prevent dropping heap while allocations that require it are live.
  - Validation:
    - Type signatures encode ownership and lifetime rules.
    - Compile-fail tests prove an allocation borrowing a heap cannot outlive the heap.
    - Safe API cannot destroy the main heap.
  - Evidence:
    - No safe heap ownership model is stabilized in this milestone.
    - `docs/safe-api.md` records the required future ownership model: owned heaps, borrowed main heap handles, and heap-tied allocation lifetimes.
    - `rg -n "pub (struct|enum|type) (Heap|MainHeap|HeapRef|HeapAllocation)|pub fn .*Heap|pub mod heap|Heap::" src/lib.rs src/safe.rs README.md` finds no public safe heap facade.

- [x] Decide heap thread-safety and mobility.
  - Required behavior:
    - `Heap` is `!Send`/`!Sync` unless cross-thread heap use is proven safe for the wrapper semantics.
    - If `Send` is allowed, docs explain thread ownership, remote frees, and deletion requirements.
    - If `Sync` is allowed, docs explain concurrent method safety.
  - Validation:
    - Trait tests prove intended `Send`/`Sync` behavior.
    - Unsafe audit contains proof for any manual impl.
  - Evidence:
    - Because no safe heap type is exposed, there are no safe heap `Send`/`Sync` impls to prove.
    - `docs/safe-api.md` requires any future safe heap design to make thread mobility and concurrent access explicit trait decisions.

## D2. Implement safe heap allocation without wrong-heap or stale-handle hazards

- [x] Add `Heap::new() -> Result<Heap, HeapError>` if safe heap creation is in scope.
  - Validation:
    - Null result maps to `HeapError`.
    - Drop deletes heap exactly once.
    - Tests cover create/drop in default, secure, and guarded features where feasible.
  - Evidence:
    - Safe heap creation is not in scope for this milestone; there is no public `Heap::new`.
    - `docs/safe-api.md` states that `Heap::new` is intentionally absent until the safe heap design is completed.

- [x] Add heap allocation methods only with lifetime-safe ownership.
  - Candidate methods:
    - `Heap::allocate(&self, layout: Layout) -> Result<HeapAllocation<'_>, AllocError>`.
    - `Heap::allocate_zeroed(&self, layout: Layout) -> Result<HeapAllocation<'_>, AllocError>`.
  - Validation:
    - Returned allocation cannot outlive heap.
    - Drop order is enforced by Rust lifetimes.
    - Tests verify heap containment where raw API supports it.
  - Evidence:
    - No safe heap allocation methods are exposed.
    - `docs/safe-api.md` requires any future safe heap allocation API to encode heap/allocation lifetimes.

- [x] Prevent freeing through wrong heap in safe APIs.
  - Required behavior:
    - Heap-owned safe allocation carries enough lifetime/ownership information to prevent misuse.
    - Safe API does not expose `heap.free(ptr)` accepting arbitrary raw pointers.
  - Validation:
    - Compile-fail tests show users cannot construct a `HeapAllocation` from a pointer without unsafe code.
    - Compile-fail tests show users cannot pass allocation from one heap into another heap's safe free API.
  - Evidence:
    - There is no safe `HeapAllocation` type and no safe `heap.free(ptr)` API accepting arbitrary raw pointers.
    - `scripts/check-api-inventory.py` passes and would fail on a new crate-root safe heap item until inventoried.

- [x] Define safe heap `collect` semantics.
  - Required docs:
    - What `force` means if exposed.
    - Whether method can block.
    - Whether it can run concurrently with allocations.
    - Whether it may call callbacks.
  - Validation:
    - Tests call collect with live and no live allocations.
    - Docs include panic and allocation behavior.
  - Evidence:
    - Safe heap `collect` is intentionally absent with the rest of the safe heap API.
    - Raw collect behavior remains under `raw`; safe semantics will be documented if a safe heap API is added.

- [x] Do not expose safe `heap_destroy` unless live allocations are impossible or semantics are fully documented.
  - Required behavior:
    - Destructive operations that invalidate outstanding allocations are unsafe or absent.
    - Safe `Drop` uses the least-surprising C operation consistent with lifetime model.
  - Validation:
    - API review confirms no safe method can invalidate safe live allocations.
    - If unsafe destroy exists, it has `# Safety` docs and examples avoiding misuse.
  - Evidence:
    - There is no safe heap destroy API and no safe heap type.
    - `docs/safe-api.md` says borrowed main-heap handles must not be deletable and owned heap deletion semantics require a future design.

## D3. Raw heap interop

- [x] Add `Heap::as_raw` only if needed.
  - Required docs:
    - Borrowed raw pointer is valid only while `Heap` lives.
    - Caller must not delete/destroy it.
    - Caller must not use it from unsupported threads.
  - Validation:
    - Method is safe only if it cannot violate ownership by itself; otherwise mark unsafe.
    - Docs include aliasing and lifetime requirements.
  - Evidence:
    - No safe `Heap::as_raw` exists because no safe heap wrapper exists.
    - `docs/safe-api.md` requires future raw interop methods to avoid accidental deletion-right transfer.

- [x] Add `unsafe Heap::from_raw` only if ownership transfer is needed.
  - Required docs:
    - Pointer must be non-null.
    - Pointer must originate from this allocator.
    - Pointer must be uniquely owned.
    - Pointer must not be main heap unless the type represents borrowed main heap.
    - Caller must guarantee no other wrapper will delete it.
  - Validation:
    - Unsafe docs include all bullets.
    - Tests cover round-trip `into_raw`/`from_raw` if both exist.
  - Evidence:
    - No safe `Heap::from_raw` exists because no safe heap wrapper exists.
    - `docs/safe-api.md` requires future raw interop methods to avoid accidental deletion-right transfer.

---

# Workstream E: Optional safe stats, options, and process information

These wrappers should be added only if they can avoid dangling references, global mutable surprises, and allocator recursion.

## E1. Safe stats snapshot

- [x] Decide whether to expose `StatsSnapshot`.
  - Required decision:
    - If yes, it must be an owned/copy snapshot, not references into live allocator metadata.
    - If no, raw stats remain under `raw` and docs say safe stats are not stabilized.
  - Validation:
    - Decision is recorded in `docs/api-inventory.md`.
    - README examples match decision.
  - Evidence:
    - `docs/api-inventory.md`, `docs/safe-api.md`, and README state that safe stats wrappers are intentionally deferred.
    - README examples only show `MiMalloc`, `Allocation`, and raw C-shaped calls; they do not present stats as safe Rust API.

- [x] Implement stats snapshot without dangling references.
  - Required behavior:
    - Snapshot owns copied numeric data.
    - Snapshot methods do not allocate unless clearly documented and outside allocator-critical paths.
    - JSON/string rendering is optional and `std`-gated or caller-buffer based.
  - Validation:
    - Tests collect stats before/after allocations and verify monotonic fields where meaningful.
    - Drop of heaps/allocations cannot invalidate snapshot.
    - Miri/model tests cover pure formatting logic if implemented.
  - Evidence:
    - No safe `StatsSnapshot` type or stats methods are exposed.
    - `docs/safe-api.md` requires any future safe stats API to return owned numeric snapshots without references into allocator metadata.
    - Snapshot collection/formatting tests do not apply until a safe stats API exists.

- [x] Define stats reset/merge behavior if exposed.
  - Required docs:
    - Whether stats are process-wide, heap-local, thread-local, or snapshot-only.
    - Whether reset affects global allocator state.
    - Concurrency behavior.
  - Validation:
    - Tests cover merge/reset semantics against raw API parity.
    - Docs include feature and platform caveats.
  - Evidence:
    - Safe stats reset/merge is not exposed.
    - `docs/safe-api.md` names process-wide, heap-local, thread-local, reset, and merge semantics as required future design points.

## E2. Safe options API

- [x] Decide whether to expose safe options.
  - Required decision:
    - No safe options API until initialization/order/threading semantics are specified.
    - Or expose a narrow read-only options view.
    - Or expose typed setters only for options proven safe to mutate at runtime.
  - Validation:
    - Decision is documented.
    - No safe option setter exists for options that can break allocator invariants when changed at the wrong time.
  - Evidence:
    - `docs/api-inventory.md`, `docs/safe-api.md`, and README state that safe options wrappers are intentionally deferred.
    - No safe `Options` type or option setter exists in the crate-root safe facade.

- [x] If options are exposed, define typed option keys and values.
  - Required behavior:
    - No raw `mi_option_t` in safe APIs.
    - No ambiguous bool/int setters.
    - Invalid values return `OptionsError`.
    - Runtime mutability is documented per option.
  - Validation:
    - Unit tests cover valid and invalid values.
    - Feature-specific options are cfg-gated and documented.
    - Raw C option behavior remains available under `raw`.
  - Evidence:
    - Safe options are not exposed, so typed safe option keys and values are intentionally absent.
    - Raw C option behavior remains under `raw`.

- [x] Prevent allocator recursion in option diagnostics.
  - Required behavior:
    - Safe option printing/logging does not allocate through the allocator unless explicitly outside allocator-critical context.
    - Callback/output behavior is raw unless a safe callback wrapper exists.
  - Validation:
    - `tests/no_allocation_critical_paths.rs` or source audit covers option diagnostics.
  - Evidence:
    - No safe option diagnostics API exists.
    - `docs/safe-api.md` requires any future safe option diagnostics to avoid allocator recursion in allocator-critical contexts.
    - Existing allocator critical-path coverage remains in `tests/no_allocation_critical_paths.rs`.

## E3. Safe process info

- [x] Decide whether to expose `ProcessInfo`.
  - Required behavior if implemented:
    - Snapshot owns numeric values.
    - OS/platform unsupported fields are `Option` or a typed unsupported error.
    - Raw out-pointer behavior remains under `raw`.
  - Validation:
    - Tests call process info under Linux x86_64 GNU.
    - Docs specify platform support and unsupported fields.
  - Evidence:
    - `docs/api-inventory.md`, `docs/safe-api.md`, and README state that safe process-info wrappers are intentionally deferred.
    - `docs/safe-api.md` requires any future safe process-info API to return owned numeric values and represent unsupported fields with `Option` or typed errors.
    - Raw out-pointer behavior remains under `raw`.

---

# Workstream F: Unsafe API and raw pointer escape hatches

## F1. Unsafe Rust API classification

- [x] Inventory every public unsafe Rust item outside `raw`.
  - Required behavior:
    - Prefer zero public unsafe items outside `raw` unless they are part of a deliberate wrapper design.
    - Every public unsafe item has a `# Safety` section.
    - Every public unsafe item is classified in API inventory.
  - Validation:
    - Script or rustdoc JSON inventory finds public unsafe items.
    - `cargo clippy --all-targets -- -D clippy::missing_safety_doc` passes or a staged allowlist is documented.
  - Evidence:
    - `scripts/check-safe-boundary.py` inventories `src/lib.rs` and `src/safe.rs` and fails on public unsafe Rust items outside the raw facade.
    - `scripts/check-safe-boundary.py` passes.
    - `docs/unsafe-audit.md` documents the staged safety-doc posture for the raw facade and safe boundary.

- [x] Add naming discipline for unsafe raw ownership transfers.
  - Required names:
    - `as_raw` for borrowed raw pointer.
    - `into_raw` for ownership transfer out.
    - `from_raw` for checked unsafe ownership transfer in.
    - `from_raw_unchecked` only if a checked version exists or cannot exist.
  - Validation:
    - Public API review finds no ad-hoc unsafe constructor names.
    - Docs define ownership transfer for each method.
  - Evidence:
    - `docs/safe-api.md` defines `as_raw`, `into_raw`, `from_raw`, and `from_raw_unchecked` naming discipline.
    - Current safe facade has no raw ownership-transfer methods; `Allocation` exposes only non-owning `as_ptr` and `as_non_null`.
    - `scripts/check-safe-boundary.py` passes and finds no public unsafe ownership-transfer item in the safe facade.

- [x] Define invalidation rules for every raw pointer returned by safe types.
  - Required docs:
    - What invalidates pointer.
    - Whether pointer may be null.
    - Whether pointer may be freed by caller.
    - Whether pointer may be used after move/drop.
    - Aliasing and thread restrictions.
  - Validation:
    - Rustdoc review for all raw pointer escape hatches.
  - Evidence:
    - `Allocation::as_ptr` and `Allocation::as_non_null` rustdoc document lifetime, no manual free, aliasing, uninitialized-read restrictions, and zero-sized sentinel behavior.
    - There are no other raw pointer escape hatches on safe public types.

## F2. Unsafe audit integration

- [x] Expand `docs/unsafe-audit.md` for safe API/FFI boundary.
  - Required sections:
    - C ABI exported functions.
    - Raw Rust facade.
    - `MiMalloc` `GlobalAlloc` impl.
    - `Allocation` wrapper.
    - Future `Heap` wrapper if implemented.
    - Future stats/options/callback wrappers if implemented.
    - Panic/unwind boundary.
    - Raw pointer escape hatches.
  - Validation:
    - Each section lists caller obligations, callee guarantees, aliasing, lifetime, alignment, initialization, concurrency, and unwind behavior.
    - Source comments link to audit sections or use stable audit tags.
  - Evidence:
    - `docs/unsafe-audit.md` includes sections for C ABI exports, raw Rust facade, `MiMalloc`, `Allocation`, deferred heap/stats/options/callback wrappers, panic/unwind boundary, and raw pointer escape hatches.
    - Safe facade source comments use stable `SAFETY[AUDIT:*]` tags.

- [x] Add audit tags to boundary unsafe blocks.
  - Example tags:
    - `SAFETY[AUDIT:global-alloc]`
    - `SAFETY[AUDIT:owned-allocation]`
    - `SAFETY[AUDIT:ffi-export]`
    - `SAFETY[AUDIT:raw-callback]`
    - `SAFETY[AUDIT:heap-wrapper]`
  - Validation:
    - Source audit fails if new boundary unsafe block lacks a `SAFETY` comment.
    - Audit doc explains tag meanings.
  - Evidence:
    - `MiMalloc`, `Allocation`, and panic-boundary unsafe blocks in the safe facade carry `SAFETY[AUDIT:*]` comments.
    - `scripts/check-safe-boundary.py` fails if safe-facade unsafe blocks or unsafe impls lack audit tags.
    - `docs/unsafe-audit.md` explains `global-alloc`, `owned-allocation`, `ffi-export`, `raw-callback`, `panic-boundary`, and reserved `heap-wrapper` tags.

- [x] Deny broad unsafe-op suppressions.
  - Required behavior:
    - Crate or Cargo lint denies `unsafe_op_in_unsafe_fn` where feasible.
    - Broad module-level allowances are forbidden.
    - Any local exception has a removal condition.
  - Validation:
    - `cargo clippy --all-targets -- -D warnings` passes.
    - Source audit confirms no broad `allow(unsafe_op_in_unsafe_fn)` remains.
  - Evidence:
    - `scripts/check-lints.sh` passes.
    - `rg -n "allow\\(unsafe_op_in_unsafe_fn\\)|unsafe_op_in_unsafe_fn" src tests scripts docs` finds only policy/audit references and release-audit test assertions, not source allowances.

---

# Workstream G: Examples and user experience

## G1. Safe Rust examples

- [x] Keep `examples/global_allocator.rs` minimal and idiomatic.
  - Validation:
    - `cargo run --example global_allocator --features std` passes.
    - Example uses `MiMalloc` from crate root and no raw APIs.
  - Evidence:
    - `examples/global_allocator.rs` uses crate-root `MiMalloc`, `Vec`, and `Box` only.
    - `cargo run --quiet --example global_allocator --features std` passes.

- [x] Keep `examples/allocation.rs` focused on the safe owned wrapper.
  - Validation:
    - Example uses `Allocation` and Rust errors.
    - It does not read uninitialized memory.
    - It does not use raw `mi_free`.
  - Evidence:
    - `examples/allocation.rs` uses crate-root `Allocation` and `TryAllocError`.
    - It does not read allocation contents and does not call raw `mi_free`.
    - `cargo run --quiet --example allocation --features std` passes.

- [x] Add heap example only if safe heap API is complete.
  - Validation:
    - If no safe heap API exists, `examples/heap.rs` must use `raw` and be clearly labeled unsafe/raw, or be removed until safe API exists.
    - If safe heap API exists, example avoids raw heap pointers.
  - Evidence:
    - No safe heap API exists.
    - The raw heap example is named `examples/raw_heap.rs` and comments identify it as raw/unsafe.
    - `cargo run --quiet --example raw_heap --features std` passes.

- [x] Add stats/options examples only if safe APIs exist.
  - Validation:
    - If examples use raw APIs, file names and docs say `raw`/unsafe.
    - README does not present raw examples as safe Rust API.
  - Evidence:
    - No safe stats or options API exists.
    - Raw examples are named `examples/raw_stats.rs` and `examples/raw_options.rs` and comments identify them as raw/unsafe.
    - README examples present only `MiMalloc`, `Allocation`, and explicitly unsafe raw C-shaped calls.
    - `cargo run --quiet --example raw_stats --features std` and `cargo run --quiet --example raw_options --features std` pass.

## G2. FFI/C examples

- [x] Maintain a C smoke example for cdylib usage.
  - Validation:
    - `examples/c_api.c` compiles and links in documented local command.
    - The example allocates, checks usable size/alignment where useful, and frees.
    - The example does not depend on Rust crate-root paths.
  - Evidence:
    - `examples/c_api.c` includes documented build/link commands, calls `mi_malloc`, checks `mi_usable_size`, and calls `mi_free`.
    - The example includes `<mimalloc.h>` and does not depend on Rust crate-root paths.
    - The documented C compile/link command succeeds after `cargo build --release`.

- [x] Add C++ new/delete example or explicit non-goal.
  - Validation:
    - If example exists, it validates operator new/delete and nothrow behavior.
    - If omitted, `docs/abi.md` points to parity script coverage.
  - Evidence:
    - A standalone C++ example is intentionally omitted.
    - `docs/abi.md` points to `scripts/check-cxx-operator-new-parity.py`, `scripts/check-abi-symbols.sh`, and release-fast-path checks for C++ operator new/delete behavior.

## G3. Documentation examples and compile-fail examples

- [x] Add rustdoc examples for every safe public type.
  - Validation:
    - `cargo test --doc --features std` passes.
  - Evidence:
    - `MiMalloc`, `Allocation`, `AllocError`, `LayoutError`, and `TryAllocError` have item-level rustdoc examples or explicit failure-branch guidance.
    - `scripts/check-docs.sh` passes.

- [x] Add compile-fail examples for inaccessible internals.
  - Required examples:
    - `mechanical_mimalloc::alloc` not accessible.
    - `mechanical_mimalloc::core_types` not accessible.
    - raw C function calls require `unsafe`.
    - optional: `raw::internal` unavailable by default after raw narrowing.
  - Validation:
    - Doctests pass and fail in intended way.
  - Evidence:
    - Crate docs include compile-fail examples for `mechanical_mimalloc::alloc`, grouped inaccessible implementation modules, `core_types`, and raw calls without `unsafe`.
    - `scripts/check-docs.sh` passes.

- [x] Add safety examples for raw calls.
  - Required docs:
    - Correct raw malloc/free pair.
    - Null free behavior.
    - Incorrect examples marked `compile_fail` only where compile-time catchable; use prose for runtime UB examples.
  - Validation:
    - Doctests pass.
    - Docs do not encourage raw use when safe wrapper suffices.
  - Evidence:
    - `src/raw.rs` documents a correct unsafe raw malloc/free pair and null free behavior.
    - Crate docs present safe `MiMalloc`/`Allocation` examples before raw examples.
    - `scripts/check-docs.sh` passes.

---

# Workstream H: Validation scripts and audits

## H1. API inventory enforcement

- [x] Expand `scripts/check-api-inventory.py` for root and raw modules.
  - Required checks:
    - Crate-root public item allowlist.
    - Default `raw` module allowlist.
    - Optional unstable raw-internals module allowlist.
    - Docs inventory consistency.
  - Validation:
    - Script fails when a new public root item is added without docs.
    - Script fails when a new raw module is added without docs.
  - Evidence:
    - `scripts/check-api-inventory.py` enforces the crate-root public item allowlist.
    - It separately enforces default raw module allowlist and `unstable-raw-internals` module allowlist.
    - It checks docs inventory policy phrases, `docs/abi.md` taxonomy phrases, and the C-header declaration ownership split.

- [x] Add `scripts/check-raw-facade.sh` or equivalent.
  - Required checks:
    - No accidental `pub use crate::<internal>::*` in default `raw`.
    - No default raw re-export of implementation-only modules.
    - Raw internals feature gates are present if internal modules remain available.
  - Validation:
    - Script is included in `scripts/check-local.sh`.
    - Deliberately adding `pub mod internal` to default raw causes failure.
  - Evidence:
    - `scripts/check-raw-facade.sh` enforces the default raw allowlist, unstable raw-internals allowlist, and absence of default implementation-module glob re-exports.
    - `scripts/check-local.sh` runs `scripts/check-raw-facade.sh`.
    - `scripts/check-raw-facade.sh` passes.

## H2. FFI export enforcement

- [x] Add or update script to inventory export attributes.
  - Search targets:
    - `#[unsafe(no_mangle)]`
    - `#[export_name = ...]`
    - `global_asm!` public symbols
    - `#[unsafe(naked)]` exported functions
    - linker version script public patterns
  - Validation:
    - Script maps each export to C ABI, malloc override, C++ override, or internal-classified exception.
    - Script fails on unclassified new export.
  - Evidence:
    - `scripts/check-ffi-boundary.py` classifies export-attribute files as `c-abi`, `malloc-cxx-override`, `runtime-boundary`, `panic-runtime-boundary`, or `c-abi-internal-classified`.
    - The script searches `#[unsafe(no_mangle)]`, `#[unsafe(export_name = ...)]`, `global_asm!`, `#[unsafe(naked)]`, and `naked_asm!`.
    - The script verifies the Linux linker version script public patterns in `build.rs` and passes.

- [x] Enforce no internal helper regains dynamic export accidentally.
  - Validation:
    - Existing release fast-path audit or new script covers known helpers.
    - `scripts/check-abi-symbols.sh` catches extra dynamic symbols.
  - Evidence:
    - `scripts/check-ffi-boundary.py` fails if export attributes appear outside classified boundary files.
    - `tests/release_fast_path_audit.rs` guards known internal helpers against regaining `#[unsafe(no_mangle)]`.
    - `scripts/check-abi-symbols.sh` passes and catches extra dynamic symbols.

## H3. Rustdoc and lints

- [x] Add Cargo `[lints]` policy or crate-level lints for safe API/FFI boundary.
  - Desired lints:
    - `unsafe_op_in_unsafe_fn = deny`
    - `rustdoc::broken_intra_doc_links = deny`
    - `rustdoc::bare_urls = warn`
    - `missing_docs = warn` after raw narrowing.
    - `clippy::missing_safety_doc = deny` after unsafe docs are complete.
    - `clippy::undocumented_unsafe_blocks = deny` after staged cleanup.
  - Validation:
    - `cargo clippy --all-targets -- -D warnings` passes.
    - Maximal valid feature-set Clippy runs pass.
  - Evidence:
    - `Cargo.toml` centrally denies `unsafe_op_in_unsafe_fn`, denies broken rustdoc links, warns on bare rustdoc URLs, and denies `clippy::missing_safety_doc`.
    - `missing_docs` remains deferred until raw narrowing so broad compatibility modules do not turn transitional public raw paths into a permanent docs contract.
    - `clippy::undocumented_unsafe_blocks` remains deferred for the mechanical port; an explicit trial currently reports existing unsafe-block audit debt rather than a clean enforceable policy.
    - `scripts/check-lints.sh` passes the default, bench, and maximal valid feature-set Clippy runs.

- [x] Ensure docs fail on broken links and doctest regressions.
  - Validation:
    - `scripts/check-docs.sh` passes.
    - CI/manual workflow, if used, runs the same command.
  - Evidence:
    - `src/lib.rs` denies `rustdoc::broken_intra_doc_links` and warns on `rustdoc::bare_urls`.
    - `scripts/check-docs.sh` runs `cargo doc --no-deps --features std` and `cargo test --doc --features std`.
    - `scripts/check-local.sh` runs `scripts/check-docs.sh`.
    - `scripts/check-docs.sh` passes.

## H4. Compile-fail tests

- [x] Add compile-fail tests for public boundary regressions.
  - Cases:
    - Old crate-root modules unavailable.
    - `core_types` unavailable.
    - Unsafe raw functions cannot be called without unsafe block.
    - Safe `Allocation` cannot be copied.
    - Heap allocation cannot outlive heap if safe heap API exists.
    - Main heap cannot be destroyed through safe API if safe heap API exists.
  - Validation:
    - Tests run through rustdoc compile-fail examples or `trybuild`.
    - These tests are part of local check script.
  - Evidence:
    - Crate docs include compile-fail examples for old crate-root modules, `core_types`, and unsafe raw calls without an unsafe block.
    - `Allocation` rustdoc includes a compile-fail use-after-move example proving the wrapper is not `Copy`.
    - Safe heap API is intentionally absent, so heap lifetime/main-heap compile-fail cases do not apply yet.
    - `scripts/check-docs.sh` runs these rustdoc compile-fail tests and is part of `scripts/check-local.sh`.

## H5. ABI/layout tests

- [ ] Ensure raw split does not change C ABI.
  - Validation:
    - `scripts/check-abi-symbols.sh` passes before and after raw split.
    - `scripts/check-layout.sh` passes before and after raw split.
    - Public C examples compile and run.
    - Any intentional dynamic symbol change is in changelog and ABI docs.

- [ ] Add feature-specific ABI checks where layout or symbols can change.
  - Required feature sets:
    - default
    - `secure`
    - `guarded`
    - `track-valgrind`
    - `track-asan`
  - Validation:
    - Scripts run or explicitly document why a feature does not affect ABI/layout.

---

# Workstream I: Migration plan from current broad raw facade

## I1. Inventory current raw users

- [x] List all uses of `mechanical_mimalloc::raw::*` in tests, benches, examples, and docs.
  - Validation:
    - Produce a table with caller file, raw path used, reason, and migration target.
    - Check the table into `docs/api-inventory.md` or a raw migration doc.
  - Evidence:
    - `docs/raw-facade-migration.md` lists current raw users across README, crate docs, examples, benches, tests, and `src/abi.rs` macros with reason and migration target.
    - `docs/api-inventory.md` links to the migration inventory.

- [x] Classify each current raw module.
  - Categories:
    - Public C header surface.
    - Public C ABI implementation/testing convenience.
    - Internal helper required by tests only.
    - Internal helper required by benchmarks only.
    - Accidental leftover.
  - Validation:
    - Every `pub mod` inside `raw` has a category.
    - Accidental leftovers are removed.
  - Evidence:
    - `docs/raw-facade-migration.md` classifies each current `raw` module as public C header surface, public C ABI implementation/testing convenience, internal helper, platform/internal implementation, or compatibility macro/type surface.
    - No module is classified as an accidental leftover yet; narrowing/gating remains tracked by the following I1/I2 and B2 items.

- [x] Migrate tests that only need behavior to public APIs.
  - Validation:
    - Tests use safe API where testing safe API.
    - Tests use `raw::abi`/public C API where testing C behavior.
    - Tests use crate-private unit tests for allocator internals.
  - Evidence:
    - Public C behavior in `original_main`, `original_main_static_dep`, `original_test_stress`, stats/lifecycle tests, and the public allocation portions of upstream API/override tests now uses root `raw::mi_*` C-header functions and C-header types/constants.
    - `src/abi.rs` exposes the public C constants needed by those tests (`MI_BIN_HUGE`, `MI_MAX_ALIGN_SIZE`) through the raw C-header facade.
    - Remaining `raw::{internal,bitmap,bits,page,track,types,headers}` test uses are internal-invariant, debug/tracking, source-shape, or C++ header-placeholder checks tracked for unstable-internals gating/crate-private movement.
    - `cargo test --quiet --features std` passes.

- [x] Migrate benchmarks to stable surfaces.
  - Validation:
    - Benchmark harnesses call safe `MiMalloc` or public C ABI/raw C-header functions.
    - They do not require `raw::internal` by default.
  - Evidence:
    - `benches/hotpath.rs` and `benches/workloads.rs` use root `raw::mi_*` C-header functions instead of `raw::alloc`, `raw::alloc_aligned`, or `raw::free` modules.
    - `cargo check --benches --features bench` passes.
    - `scripts/check-bench-smoke.sh` passes.

## I2. Provide transition compatibility if needed

- [x] Decide whether to keep temporary raw internals under a feature.
  - Required docs if kept:
    - Feature name.
    - No semver guarantees.
    - Intended users: repository tests/benches only or advanced low-level users.
    - Removal conditions.
  - Validation:
    - Feature is off by default.
    - Docs and changelog warn about instability.
  - Evidence:
    - Temporary raw internals are kept under `unstable-raw-internals`.
    - The feature is off by default and not implied by `std`, `bench`, or `compare`.
    - Docs warn that the feature is repository-audit/parity-test only and has no SemVer guarantees.

- [x] Add deprecation warnings for old raw paths if they remain public temporarily.
  - Required behavior:
    - Use `#[deprecated]` where possible.
    - Deprecation message points to replacement path or says “not public API.”
  - Validation:
    - `cargo check` under compatibility feature shows expected deprecation warnings in a controlled test.
    - Docs mark deprecated modules.
  - Evidence:
    - Every `unstable-raw-internals` module in `src/raw.rs` is annotated `#[deprecated]` with a note that the path is not public API and callers should use root `raw::mi_*`/`raw::abi` for C-header surface.
    - `cargo test --quiet --features "std unstable-raw-internals"` exercises those paths and emits controlled deprecation warnings without failing.

## I3. Final raw facade cutover

- [x] Remove or gate broad re-exports.
  - Validation:
    - Default `raw.rs` exposes only allowlisted modules.
    - Examples/docs updated.
    - All local scripts pass.
  - Evidence:
    - Broad translated implementation modules are gated behind `unstable-raw-internals` in `src/raw.rs`.
    - Default raw facade remains limited to the allowlisted C-header modules.
    - `scripts/check-raw-facade.sh` enforces this shape.

- [ ] Update public API inventory and changelog.
  - Validation:
    - `docs/api-inventory.md` matches rustdoc.
    - `CHANGELOG.md` classifies this as raw Rust path cleanup, not C ABI removal unless symbols changed.

---

# Workstream J: Safe API testing matrix

## J1. Safe facade unit and integration tests

- [ ] Test `MiMalloc` as global allocator.
  - Cases:
    - `Box`, `Vec`, `String` under `std`.
    - Multi-thread allocation/free.
    - Over-aligned allocation via standard allocation APIs where feasible.
  - Validation:
    - `examples/global_allocator.rs` runs.
    - Integration test installs `MiMalloc` as global allocator in a test binary.

- [ ] Test `Allocation` normal allocation.
  - Cases:
    - small sizes
    - large sizes
    - alignments 1 through at least 4096
    - zero size
    - allocation failure/overflow where feasible
  - Validation:
    - Tests verify alignment, length, non-null for nonzero, no crash on drop.

- [ ] Test `Allocation::zeroed`.
  - Cases:
    - zeroed bytes for several sizes/alignments.
    - zero size.
  - Validation:
    - Safe reads occur only for initialized/zeroed allocation state.

- [ ] Test raw pointer escape hatches.
  - Cases:
    - `as_ptr` gives writable memory while alive.
    - pointer not used after drop in safe tests.
    - docs state unsafe obligations for external use.
  - Validation:
    - Tests write/read through raw pointer only inside unsafe blocks.

- [ ] Test safe API under features.
  - Feature sets:
    - default
    - `std`
    - `secure`
    - `guarded`
    - `track-valgrind`
    - `track-asan`
  - Validation:
    - `scripts/check-features.sh` and targeted safe API tests pass.

## J2. Safe heap tests, if heap is implemented

- [ ] Test heap create/drop.
  - Validation:
    - No leaks or double frees under debug/sanitizer checks where feasible.

- [ ] Test heap allocation lifetimes.
  - Validation:
    - Compile-fail test proves allocation cannot outlive heap.
    - Runtime tests verify heap containment.

- [ ] Test heap non-Send/non-Sync or Send/Sync behavior.
  - Validation:
    - Static trait tests verify intended behavior.
    - Unsafe audit proof exists for manual impls.

## J3. Safe stats/options tests, if implemented

- [ ] Test stats snapshots.
  - Validation:
    - Snapshot remains valid after allocations/frees.
    - Snapshot contains plausible deltas.
    - No references into allocator metadata escape.

- [ ] Test options API.
  - Validation:
    - Valid values set/read correctly.
    - Invalid values return typed errors.
    - Unsupported runtime changes are rejected safely.

---

# Workstream K: Documentation deliverables

## K1. Required docs files

- [x] `docs/safe-api.md` or equivalent exists.
  - Required contents:
    - Safe API philosophy.
    - Stable/experimental status.
    - `MiMalloc` docs.
    - `Allocation` docs.
    - Heap/stats/options decision.
    - Error model.
    - Panic/OOM behavior.
    - Threading behavior.
    - Feature interactions.
  - Validation:
    - README links to it.
    - Crate docs link to it.

- [ ] `docs/ffi-raw-split.md` or expanded `docs/abi.md` exists.
  - Required contents:
    - Module taxonomy.
    - What belongs in `raw`.
    - What belongs in FFI/export wrappers.
    - What remains internal.
    - Raw facade stability.
    - C ABI compatibility.
    - Symbol and layout validation.
  - Validation:
    - New contributors can identify where to put a new C ABI declaration or safe wrapper.

- [ ] `docs/unsafe-audit.md` includes safe API / FFI split proofs.
  - Validation:
    - Each public safe wrapper has a safety proof section.
    - Each public unsafe/raw family has caller obligations.

- [ ] `docs/api-inventory.md` is complete.
  - Required inventory:
    - Crate-root safe API.
    - Raw default API.
    - Raw unstable/internal feature API if any.
    - C ABI symbols/layouts.
    - Hidden internals.
    - SemVer classification.
  - Validation:
    - Inventory script passes.

## K2. README updates

- [ ] README accurately distinguishes safe, raw, and C ABI usage.
  - Validation:
    - Safe examples use only crate-root safe APIs.
    - Raw examples are wrapped in `unsafe` and labeled raw/unsafe.
    - C usage is described separately from Rust raw usage.

- [ ] README documents what is not stabilized.
  - Required notes:
    - Safe heap API status.
    - Safe stats/options status.
    - Raw facade stability status.
    - Platform support limits.
  - Validation:
    - README matches `docs/limitations.md` and `docs/api-inventory.md`.
  - Evidence:
    - `docs/safe-api.md` exists and documents safe API philosophy plus stable/experimental status.
    - README, `docs/limitations.md`, and `docs/api-inventory.md` all describe the safe API as narrow/limited to the current facade rather than a broad heap/options/stats abstraction.

## K3. Changelog and migration notes

- [ ] Changelog classifies safe API / raw split changes.
  - Required sections under Unreleased:
    - Added
    - Changed
    - Deprecated
    - Removed
    - Safety
    - ABI
    - Performance
    - Documentation
  - Validation:
    - Raw path removals/gates are listed.
    - Safe API additions are listed.
    - C ABI unchanged or changes are explicit.

- [ ] Add migration notes for old raw paths.
  - Required contents:
    - Old path.
    - New path.
    - Whether it is safe, raw public, unstable internals, or removed.
  - Validation:
    - Tests/examples/docs use new paths.

---

# Workstream L: Final exemplary-state acceptance review

## L1. Reviewer questions must be answerable from docs

- [ ] A reviewer can answer: What is the stable safe Rust API?
  - Validation:
    - Answer appears in README and `docs/api-inventory.md`.

- [ ] A reviewer can answer: What is the raw Rust facade, and is it stable?
  - Validation:
    - Answer appears in `docs/api-inventory.md` and `docs/compatibility.md`.

- [ ] A reviewer can answer: What is the C ABI compatibility surface?
  - Validation:
    - Answer appears in `docs/abi.md` and symbol scripts.

- [ ] A reviewer can answer: Where do new C ABI declarations go?
  - Validation:
    - Answer appears in `docs/ffi-raw-split.md` or `docs/abi.md`.

- [ ] A reviewer can answer: Where do new safe wrappers go?
  - Validation:
    - Answer appears in `docs/safe-api.md` or `docs/architecture.md`.

- [ ] A reviewer can answer: What are the unsafe obligations for raw API users?
  - Validation:
    - Answer appears in `raw` rustdoc and `docs/unsafe-audit.md`.

- [ ] A reviewer can answer: What platforms are supported?
  - Validation:
    - Answer appears in README and `docs/platforms.md`.

- [ ] A reviewer can answer: How do I verify the split locally?
  - Validation:
    - Answer appears in README, `docs/testing.md`, and `docs/release.md`.

## L2. Final command signoff

Before marking this TODO complete, run and record the exact output/status for:

- [ ] `cargo fmt --check`
  - Validation: exits 0.

- [ ] `cargo test --quiet`
  - Validation: exits 0.

- [ ] `cargo test --doc --features std`
  - Validation: exits 0.

- [ ] `cargo check --examples --features std`
  - Validation: exits 0.

- [ ] `scripts/check-api-inventory.py`
  - Validation: exits 0 and reports expected crate-root/raw inventory.

- [ ] `scripts/check-raw-facade.sh` or equivalent
  - Validation: exits 0 and reports no accidental raw internals in default facade.

- [ ] `scripts/check-features.sh`
  - Validation: exits 0; invalid `track-valgrind track-asan` combination fails as expected.

- [ ] `scripts/check-lints.sh`
  - Validation: exits 0 for default, bench, and maximal valid feature sets.

- [ ] `scripts/check-docs.sh`
  - Validation: exits 0.

- [ ] `scripts/check-layout.sh`
  - Validation: exits 0.

- [ ] `scripts/check-abi-symbols.sh`
  - Validation: exits 0.

- [ ] `scripts/check-package.sh`
  - Validation: exits 0 and package contents match policy.

- [ ] `scripts/check-bench-smoke.sh`
  - Validation: exits 0 for changes that could affect hot paths; documentation-only/raw path changes may record why smoke/full bench is not required.

- [ ] `scripts/check-bench-full.sh`
  - Validation: required before release or any allocator-wide/hot-path change; report is appended to `BENCHMARK_RESULTS.md` with exact commit hash.

## L3. Final API surface diff

- [ ] Generate and review public API diff from before and after this workstream.
  - Validation:
    - Removed crate-root internals are intentional.
    - Safe API additions are documented.
    - Raw path changes are documented as unstable raw facade cleanup, not C ABI removal.
    - C ABI symbols are unchanged unless intentionally documented.

- [ ] Generate and review rustdoc output.
  - Validation:
    - Crate landing page shows safe API first.
    - Raw API is visibly unsafe/low-level.
    - Internal modules are absent from root docs.
    - No broken links.

- [ ] Generate and review dynamic symbol diff.
  - Validation:
    - No mimalloc-compatible C ABI symbols are removed.
    - No internal Rust symbols become newly exported.
    - Any extra public symbol is classified.

## L4. Final “exemplary” checklist

This TODO is complete when all answers are “yes”:

- [ ] Is the crate-root safe Rust API small, documented, and intentional?
  - Validation: root rustdoc and API inventory agree.

- [ ] Can safe Rust users use the allocator without learning the mechanical C-port module graph?
  - Validation: README quick start uses only `MiMalloc` and safe wrappers.

- [ ] Is every raw/unsafe API visibly raw/unsafe and documented with caller obligations?
  - Validation: raw docs and unsafe audit cover all raw families.

- [ ] Is the C ABI separated from Rust semver?
  - Validation: compatibility docs define separate domains.

- [ ] Is the raw Rust facade no longer an accidental dump of implementation modules?
  - Validation: raw facade allowlist and docs enforce this.

- [ ] Can new contributors tell where to add safe APIs, raw declarations, exported symbols, and internals?
  - Validation: architecture/ABI docs answer this.

- [ ] Are public examples safe by default and unsafe only when explicitly demonstrating raw APIs?
  - Validation: examples and doctests pass.

- [ ] Are feature interactions documented and checked?
  - Validation: feature docs and feature sweep pass.

- [ ] Are ABI symbols and layouts validated after the split?
  - Validation: ABI/layout scripts pass.

- [ ] Are unsafe boundaries reviewed and auditable?
  - Validation: lint policy, safety comments, and unsafe-audit docs pass review.

---

# Recommended implementation order

1. Add raw facade allowlist and raw inventory script.
2. Classify every current `raw` re-export.
3. Gate or remove raw implementation-module re-exports.
4. Update tests, benches, and examples to use safe API, public C raw API, or crate-private internals appropriately.
5. Expand ABI docs to show final module taxonomy.
6. Add hard lint policy for unsafe-op and safety docs where feasible.
7. Deepen `docs/unsafe-audit.md` for safe API and FFI boundary proofs.
8. Finalize `Allocation` initialization model.
9. Decide safe `Heap`/`Stats`/`Options` scope: implement sound wrappers or explicitly defer.
10. Add compile-fail tests for public boundary rules.
11. Run full local signoff.
12. Append current exact-commit evidence to changelog and benchmark docs if behavior or hot paths moved.

# Suggested final state

At the end of this work, the public shape should feel like this:

```rust
// Safe Rust facade
pub struct MiMalloc;
pub struct Allocation;              // or finalized initialized/uninitialized split
pub struct AllocError;
pub struct LayoutError;
pub enum TryAllocError;

// Optional only if fully completed
pub struct Heap;
pub struct HeapAllocation<'heap>;
pub struct StatsSnapshot;
pub struct ProcessInfo;
pub struct Options;

// Unsafe/raw compatibility
pub mod raw {
    pub mod abi;
    pub mod mimalloc; // or c_api
    // no implementation internals by default
}
```

The dynamic `cdylib` should continue to export the mimalloc-compatible C ABI, while the Rust crate should no longer make the mechanical translation graph part of the normal public API.
