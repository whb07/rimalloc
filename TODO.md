# TODO: Release Readiness Backlog

Source: local validation of the 2026-06 release-readiness review against the
current checkout. This backlog keeps only issues that are supported by the
repository and are actionable before a production-style release.

Scope: public Rust API contract, raw facade contract, C ABI artifacts, symbol
exports, unsafe audit evidence, hardening, tests, release automation,
documentation, platform policy, and security governance.

Rules:

- Do not mark an item complete until the named code, documentation, or tooling
  change exists and its validation command passes.
- Keep Tier S safe Rust API narrow unless a separate design document and tests
  justify expanding it.
- Treat C ABI, raw Rust paths, feature flags, macros, MSRV, and supported
  targets as compatibility-sensitive surfaces.
- Preserve hot-path behavior. Any allocator-core or strict-type change needs
  the benchmark evidence required by `docs/performance.md`.
- For Rust code changes, run the project-required Rust verification before
  declaring completion: `cargo fmt --all`, `cargo clippy --workspace
  --all-targets --all-features`, and `cargo test --workspace --all-targets
  --all-features`.

## Validation notes

These review concerns are already addressed in the current checkout and are not
repeated as TODO work:

- The crate root is already narrow and implementation modules are private by
  default.
- The Tier S, Tier U, Tier R, Tier C, and Tier I classification exists in
  `docs/api-inventory.md` and `docs/compatibility.md`.
- `unsafe_callbacks` is already separate from the crate-root Tier S facade.
- `raw` is already documented as unsafe and unstable as a Rust path surface.
- `unstable-raw-internals` is already off by default and gates translated
  implementation modules.
- `Cargo.toml` currently declares `[lib] crate-type = ["rlib"]`; cdylib and
  staticlib artifacts are selected explicitly with `cargo rustc`.
- C++ override and TLS shims are already feature-gated in `build.rs`.
- Maintained target claims are already limited to Linux x86_64 GNU.
- Object ownership APIs such as `MiBox<T>` and `HeapBox<'heap, T>` are already
  explicitly deferred.
- ASan, TSan, Miri, Valgrind, and safety-stress workflows already exist; the
  remaining work is to align release policy and artifact-level coverage with
  those workflows.

Validated remaining issues:

- `build.rs` still uses a broad Linux version-script pattern (`mi_*`) and also
  exposes malloc/C++ patterns whenever `c-abi` is enabled, not only when the
  interpose/C++ override modes require them.
- `symbols/public-symbol-classification.tsv` still classifies Rust-extension
  `mi_*` symbols and a prefix `_mi_` internal static archive surface.
- Public macro exports are still large and include internal compatibility
  macros, even though they are now inventoried.
- The C API example still says `cargo build --release` builds the linkable C
  library, which conflicts with the current `rlib` default.
- The crate package excludes repo scripts and vendored C headers, so FFI users
  need a separate artifact story or explicit source-build instructions.
- There is no maintained `cargo-fuzz`, libFuzzer, or AFL target, only
  deterministic property and byte-stream fuzz-style tests.
- Unsafe audit tooling does not yet require `SAFETY[AUDIT:*]` markers for every
  unsafe operation in every source file.
- Existing C/C++/LD_PRELOAD parity helpers are not fully promoted into the
  main release checklist or validation workflow.

## P0 - Freeze the production contract

- [x] Record the production contract in `docs/release.md`.
  - [x] State that `docs/api-inventory.md` is the authoritative public API
        inventory for release signoff unless a separate `PUBLIC_API.md` is
        created.
  - [x] State that Tier S is the only stable Rust safe API surface.
  - [x] State that Tier U is public unsafe Rust API with Rust SemVer review,
        but is not Tier S.
  - [x] State that Tier R remains unsafe and Rust-path unstable unless a later
        version explicitly stabilizes it.
  - [x] State that Tier C is a C ABI contract only and does not imply Rust ABI,
        Rust layout, or Rust module-path stability.
  - [x] Add a release-signoff checklist entry that confirms no new crate-root
        public item was added without inventory and compatibility review.
  - Evidence: `docs/release.md` now has a "Production Contract" section and
    release signoff requirement for new crate-root public items.
  - Verify: `scripts/check-api-inventory.py`.

- [x] Make the pre-1.0 decision for `Options`.
  - [x] Decide whether `Options` is removed before the first stable release or
        kept as a deprecated compatibility alias.
  - [x] If removed, delete the alias, remove it from the crate-root re-export,
        update `docs/api-inventory.md`, update `docs/safe-api.md`, and add a
        changelog migration note to use `GlobalOptions`.
  - [x] If kept, add `#[deprecated]` to the alias with a replacement note,
        update examples and docs to use only `GlobalOptions`, and keep tests
        compiling with `#[allow(deprecated)]` only where compatibility is being
        tested.
        Not applicable: the pre-1.0 decision is to remove the alias.
  - [x] Update API inventory expectations in `scripts/check-api-inventory.py`.
  - Evidence: `Options` was removed from the safe facade, crate-root re-export,
    API inventory, and inventory script; `CHANGELOG.md` directs callers to
    `GlobalOptions`.
  - Verify: `cargo test --doc --features std` and
    `scripts/check-api-inventory.py`.

- [x] Confirm the long-term shape of `raw`.
  - [x] Decide whether the current Option A is the release policy: public
        `raw`, explicitly unsafe, explicitly unstable as a Rust path surface.
  - [x] If Option A is kept, add a release note that `raw` remains available for
        C-header parity, tests, benchmarks, and low-level integrations only.
  - [x] If Option B is chosen, gate `raw` behind an explicit feature before
        the stable release and update examples, tests, docs, and inventory.
        Not applicable: Option A is the release policy.
  - [x] If Option C is chosen, create a split-crate design for a safe Rust crate
        plus a `-sys` or FFI artifact crate before implementation.
        Not applicable: Option A is the release policy.
  - Evidence: `docs/release.md` now records the Tier R raw facade release
    policy, and `CHANGELOG.md` records the release-note text.
  - Verify: `scripts/check-raw-facade.sh` and `scripts/check-docs.sh`.

- [ ] Add a release API diff artifact for Tier S and Tier U.
  - [ ] Choose either an existing tool such as `cargo public-api` or extend
        `scripts/check-api-inventory.py` to emit a checked snapshot.
  - [ ] Store the release snapshot under a stable path such as
        `docs/api-snapshots/`.
  - [ ] Add release instructions for comparing the current snapshot to the
        previous release tag.
  - [ ] Ensure raw/Tier R changes remain classified separately from Tier S/U
        Rust source compatibility.
  - Verify: release snapshot command exits cleanly and the snapshot diff is
    reviewed.

## P0 - Clean the dynamic C ABI

- [ ] Replace the broad Linux `mi_*` dynamic export pattern with an exact
      allowlist.
  - [ ] Generate the public `mi_*` allowlist from
        `mimalloc-3.3.2/include/mimalloc.h` and
        `mimalloc-3.3.2/include/mimalloc-stats.h`.
  - [ ] Include only documented public-header symbols in the default C ABI
        symbol group.
  - [ ] Keep Rust-extension `mi_*` helpers out of the default dynamic export
        set unless they are renamed or documented as extension ABI.
  - [ ] Fail the check if a new dynamic `mi_*` symbol appears without an
        explicit public-header or extension-ABI decision.
  - Verify: new dynamic symbol check plus `scripts/check-abi-symbols.sh`.

- [ ] Split dynamic export groups by artifact mode.
  - [ ] For `--features c-abi --crate-type cdylib`, export public mimalloc
        `mi_*` symbols and required public layouts/callbacks only.
  - [ ] For `--features interpose --crate-type cdylib`, add malloc-family
        override symbols.
  - [ ] For `--features cxx-overrides` or `interpose`, add Itanium C++
        operator new/delete override symbols only when the C++ shim is linked.
  - [ ] Ensure `c-abi` alone does not export malloc-family or C++ operator
        symbols by version-script pattern.
  - [ ] Add negative tests proving those symbols are absent from the wrong
        artifact modes.
  - Verify: `readelf -Ws` or `nm -D` checks for `c-abi`, `cxx-overrides`, and
    `interpose` cdylibs.

- [ ] Decide the Rust-extension symbol policy.
  - [ ] Review each `rust-extension` and `platform-helper-extension` entry in
        `symbols/public-symbol-classification.tsv`.
  - [ ] For each extra `mi_*` symbol, choose one outcome: hide, rename with a
        `mechanical_mimalloc_*` prefix, or document as intentional extension
        ABI.
  - [ ] If hidden, remove it from the dynamic export allowlist and keep only
        internal Rust reachability.
  - [ ] If renamed, provide compatibility only if the old symbol was already
        part of a promised release.
  - [ ] If documented, add ownership, nullability, threading, and stability
        text to `docs/abi.md`.
  - Verify: dynamic symbol manifest shows no unintentional extra `mi_*`
    symbols.

- [ ] Add dynamic ABI golden manifests.
  - [ ] Produce one checked manifest for `c-abi` cdylib exports.
  - [ ] Produce one checked manifest for `interpose` cdylib exports.
  - [ ] Include symbol names, binding, visibility, version where available, and
        artifact build command.
  - [ ] Add a script that diffs current output against the checked manifest.
  - [ ] Document how to intentionally update the manifest.
  - Verify: manifest check fails on an injected extra symbol in a local
    self-test or fixture.

- [ ] Add ABI diff checks between releases.
  - [ ] Define where previous release symbol manifests are stored or fetched.
  - [ ] Compare new cdylib and staticlib manifests against the previous release.
  - [ ] Classify removed, added, renamed, and visibility-changed symbols.
  - [ ] Require a changelog entry for every intentional ABI change.
  - Verify: release checklist includes the ABI diff command.

## P0 - Decide and clean the staticlib story

- [ ] Decide whether staticlib is production-supported or preview-only.
  - [ ] If production-supported, define the exact static archive symbol
        contract.
  - [ ] If preview-only, state this in README, `docs/abi.md`,
        `docs/release.md`, and the FFI linking guide.
  - [ ] Make the decision visible in release notes and artifact names.
  - Verify: docs and release checklist use the same staticlib status.

- [ ] If staticlib is production-supported, reduce accidental symbol leakage.
  - [ ] Audit every `_mi_*` static archive export currently covered by the
        prefix classification.
  - [ ] Determine which internal C-shaped symbols are truly required by the
        static archive link model.
  - [ ] Hide, make Rust-private, or prefix internal symbols that do not need
        external linkage.
  - [ ] Add a static archive symbol manifest with expected public and internal
        groups.
  - [ ] Fail CI if a new `_mi_*`, `mi_*`, malloc-family, or C++ symbol appears
        without classification.
  - Verify: `scripts/check-public-symbols.py` plus the new static manifest
    check.

- [ ] Add static link collision tests.
  - [ ] Build a C app that links the Rust staticlib and another object/library
        with representative non-conflicting symbols.
  - [ ] Add a negative fixture for a known collision class if static leakage is
        intentionally preview-only.
  - [ ] Verify malloc-family symbols behave as documented in static link mode.
  - [ ] Verify internal `_mi_*` symbols are either absent or explicitly
        documented as not stable.
  - Verify: new static collision script in the release checklist.

## P0 - Build and ship release artifacts

- [ ] Choose the FFI artifact distribution model.
  - [ ] Decide between crates.io Rust crate only plus GitHub release assets,
        crates.io package with enough source-build instructions, or a split
        Rust/FFI crate model.
  - [ ] Document why the chosen model works even though `scripts/` and
        vendored headers are not included in the current crate package.
  - [ ] Update `docs/release.md` and README with the chosen model.
  - Verify: `scripts/check-package.sh` and a release dry-run artifact build.

- [ ] Add an explicit release artifact build script.
  - [ ] Build the Rust library artifact:
        `cargo build --release --lib`.
  - [ ] Build C ABI cdylib:
        `cargo rustc --release --features c-abi --lib --
        --crate-type cdylib`.
  - [ ] Build C ABI staticlib:
        `cargo rustc --release --features c-abi --lib --
        --crate-type staticlib`.
  - [ ] Build interpose cdylib:
        `cargo rustc --release --features interpose --lib --
        --crate-type cdylib`.
  - [ ] Build interpose staticlib:
        `cargo rustc --release --features interpose --lib --
        --crate-type staticlib`.
  - [ ] Use isolated `CARGO_TARGET_DIR` values so artifacts from different
        modes cannot be confused.
  - [ ] Emit a machine-readable manifest containing artifact path, feature set,
        target triple, profile, rustc version, and checksum.
  - Verify: artifact build script exits cleanly from a fresh checkout.

- [ ] Ship C headers with release artifacts.
  - [ ] Include `mimalloc.h`.
  - [ ] Include `mimalloc-stats.h`.
  - [ ] Decide whether to include `mimalloc-override.h` for interposition
        users.
  - [ ] Decide whether to include `mimalloc-new-delete.h` for C++ users.
  - [ ] Record the header hashes from `PARITY.md` or regenerate them in the
        artifact manifest.
  - Verify: a C consumer builds using only the staged release artifact
    directory, not the repository source tree.

- [ ] Add link metadata for FFI consumers.
  - [ ] Generate `mechanical_mimalloc.pc`.
  - [ ] Generate `mechanical_mimalloc-config.cmake`.
  - [ ] Include library search paths, include paths, pthread/dl/m link
        requirements, and artifact-mode notes.
  - [ ] Test pkg-config with a C consumer.
  - [ ] Test CMake `find_package` with a C consumer.
  - Verify: package metadata tests run from a temporary directory outside the
    repository.

- [ ] Produce release integrity files.
  - [ ] Generate `SHA256SUMS`.
  - [ ] Generate a checked ABI symbol manifest for every shipped binary
        artifact.
  - [ ] Generate an environment/reproducibility note using
        `scripts/env-info.sh`.
  - [ ] Include benchmark report references or attach the benchmark summary
        required by `docs/performance.md`.
  - Verify: release dry-run produces all expected files.

- [ ] Fix stale C API build instructions.
  - [ ] Update `examples/c_api.c`; `cargo build --release` no longer builds a
        linkable cdylib with the current `[lib] crate-type = ["rlib"]`.
  - [ ] Show the correct cdylib and staticlib commands.
  - [ ] Show the correct `cc` command for dynamic linking.
  - [ ] Show the correct `cc` command for static linking.
  - [ ] Keep the example aligned with the chosen artifact distribution model.
  - Verify: run the documented commands from a clean target directory.

## P0 - Add real C and C++ consumer tests

- [ ] Extend C public API link tests to cover release artifact modes.
  - [ ] Keep the existing static public API smoke coverage.
  - [ ] Add a dynamic C smoke test against the `c-abi` cdylib.
  - [ ] Add a static C smoke test against the `c-abi` staticlib.
  - [ ] Add a dynamic C smoke test against the `interpose` cdylib when
        interposition symbols are expected.
  - [ ] Add a static C smoke test against the `interpose` staticlib.
  - [ ] Build consumers from a temporary directory using staged headers and
        libraries.
  - Verify: new C consumer script is run by validation or release CI.

- [ ] Add focused `dlopen`/`dlsym` tests.
  - [ ] Build the documented `c-abi` cdylib.
  - [ ] `dlopen` the artifact with `RTLD_NOW`.
  - [ ] Resolve `mi_malloc`, `mi_free`, `mi_good_size`, and representative
        stats/options symbols.
  - [ ] Allocate and free through resolved function pointers.
  - [ ] Assert plain `malloc` is not redirected by `dlopen` alone.
  - [ ] Assert no unexpected extension symbol is resolvable in the clean
        default C ABI artifact.
  - Verify: focused dynamic loading script passes on Linux x86_64 GNU.

- [ ] Add focused `LD_PRELOAD` interposition tests.
  - [ ] Build the documented `interpose` cdylib.
  - [ ] Run a C program under `LD_PRELOAD` and verify `malloc`, `calloc`,
        `realloc`, `free`, `aligned_alloc`, `posix_memalign`, `memalign`,
        `valloc`, and `malloc_usable_size` route to the allocator.
  - [ ] Verify `mi_check_owned` or equivalent ownership checks where the C
        contract supports them.
  - [ ] Verify shutdown stats behavior when `mimalloc_show_stats=1`.
  - [ ] Keep this as a small CI-friendly script even if
        `scripts/check-behavioral-parity.py` remains broader.
  - Verify: focused LD_PRELOAD script is run by validation or release CI.

- [ ] Align broad behavioral parity with documented artifact modes.
  - [ ] Update `scripts/check-behavioral-parity.py` so `rust_cdylib()` builds
        the documented dynamic artifact mode, not an unfeatured cdylib unless
        that is intentionally being tested.
  - [ ] Add separate helper names for plain cdylib, `c-abi` cdylib, and
        `interpose` cdylib if all three modes remain useful.
  - [ ] Record which parity cases require interposition and which require only
        C ABI exports.
  - [ ] Add command-line flags if the full parity script remains too expensive
        for every CI run.
  - Verify: broad parity output identifies the exact artifact mode under test.

- [ ] Add C++ release artifact tests.
  - [ ] Test C++ operator new/delete overrides for the `interpose` staticlib.
  - [ ] Test C++ operator new/delete overrides for the `interpose` cdylib if
        that is a supported artifact.
  - [ ] Test nothrow, aligned, sized delete, array new/delete, and
        `std::new_handler` behavior.
  - [ ] Keep the documented bad-allocation divergence for Rust-originated
        `mi_new*` C ABI entry points.
  - Verify: `scripts/check-cxx-operator-new-parity.py` or a new artifact-mode
    variant passes for supported C++ modes.

## P0 - Reduce public macro surface

- [ ] Decide which exported macros are part of the production contract.
  - [ ] Review the `tier-r-typed-allocation` macro list.
  - [ ] Review the `tier-r-header-override` macro list.
  - [ ] Review every `internal-compatibility` macro.
  - [ ] Choose keep, feature-gate, make crate-private, or remove for each
        macro bucket before a stable release.
  - [ ] Document the decision in `docs/api-inventory.md` and `docs/abi.md`.
  - Verify: `scripts/check-api-inventory.py`.

- [ ] Remove or privatize internal compatibility macros where possible.
  - [ ] Inventory each internal macro call site.
  - [ ] Replace single-crate call sites with crate-private `macro_rules!`
        definitions when downstream expansion does not require
        `#[macro_export]`.
  - [ ] Replace simple statement-like macros with functions where that does
        not affect hot-path codegen or translated parity.
  - [ ] Keep `#[macro_export]` only where downstream expansion or C-header
        parity requires it.
  - [ ] Add release-fast-path or assembly evidence for any hot-path macro
        replacement.
  - Verify: `cargo test --workspace --all-targets --all-features` and relevant
    benchmark smoke if hot paths moved.

- [ ] Feature-gate C-header compatibility macros if they are not part of the
      default Rust contract.
  - [ ] Decide on a feature name such as `c-header-macros` if gating is chosen.
  - [ ] Ensure typed allocation macros and header override macros are available
        only in intended configurations.
  - [ ] Update examples and doctests to enable the feature when needed.
  - [ ] Update `scripts/check-api-inventory.py` to check macro inventory per
        feature set.
  - [ ] Keep compile-fail tests proving raw macros require caller-side unsafe
        blocks.
  - Verify: `cargo test --doc --features std` plus macro feature checks.

## P1 - Retire `unstable-raw-internals`

- [ ] Move internal-invariant integration tests into crate-private unit tests
      where practical.
  - [ ] Start with tests listed in `docs/raw-facade-migration.md` that use
        `raw::internal`, `raw::types`, `raw::page`, `raw::bits`, `raw::track`,
        or `raw::bitmap`.
  - [ ] Keep public C API behavior tests on root `raw::mi_*` paths.
  - [ ] Avoid exposing new public helper APIs just to satisfy tests.
  - Verify: `scripts/check-raw-facade.sh`.

- [ ] Remove the `unstable-raw-internals` feature after no repository tests
      require it.
  - [ ] Delete gated module re-exports from `src/raw/mod.rs`.
  - [ ] Remove feature references from `Cargo.toml`.
  - [ ] Update `docs/raw-facade-migration.md`, `docs/api-inventory.md`,
        `docs/features.md`, and `docs/limitations.md`.
  - [ ] Update scripts that currently expect unstable raw-internals modules.
  - Verify: `scripts/check-api-inventory.py`, `scripts/check-raw-facade.sh`,
    and the full Rust test suite.

## P1 - Finish unsafe audit coverage

- [ ] Require `SAFETY[AUDIT:*]` markers for every unsafe operation.
  - [ ] Expand `scripts/check-unsafe-audit.py` beyond the currently migrated
        file subset.
  - [ ] Detect untagged `unsafe { ... }` blocks.
  - [ ] Detect untagged `unsafe impl` blocks.
  - [ ] Detect untagged `unsafe extern` blocks and exported ABI wrappers.
  - [ ] Detect untagged naked/assembly or export-name safety boundaries where
        the source syntax permits it.
  - [ ] Provide a migration allowlist only for files with explicit owner and
        removal date.
  - Verify: `scripts/check-unsafe-audit.py`.

- [ ] Map every audit tag to tests or manual evidence.
  - [ ] Add a table to `UNSAFE_AUDIT.md` or a machine-readable audit evidence
        file.
  - [ ] For each `SAFETY[AUDIT:*]` tag, list direct tests, parity scripts,
        Miri model tests, sanitizer evidence, benchmark/assembly evidence, or
        manual proof.
  - [ ] Add a script that fails when a source audit tag has no evidence entry.
  - [ ] Add a script that fails when an evidence entry references a missing
        test or script.
  - Verify: new audit-evidence check plus `scripts/check-unsafe-audit.py`.

- [ ] Add an advisory unsafe inventory report.
  - [ ] Choose `cargo geiger` or a local equivalent.
  - [ ] Generate a report for normal, `c-abi`, `interpose`, `secure`,
        `guarded`, `track-valgrind`, and `track-asan` builds.
  - [ ] Store the release report as advisory evidence, not as the sole safety
        proof.
  - [ ] Document how new unsafe count changes are reviewed.
  - Verify: release checklist includes the unsafe inventory report command.

## P1 - Add real fuzzing and failure injection

- [ ] Add maintained external fuzz targets.
  - [ ] Create a `cargo-fuzz` workspace or an equivalent maintained fuzzing
        harness.
  - [ ] Fuzz raw allocation operation sequences: malloc, zalloc, calloc,
        realloc, aligned allocation, heap allocation, collect, and free.
  - [ ] Include nulls, zero sizes, overflow-sized requests, small sizes, large
        sizes, and alignment classes.
  - [ ] Include per-heap ownership, cross-thread free, realloc preservation,
        zeroing, usable-size lower bounds, and stats consistency.
  - [ ] Add corpus seeds derived from `tests/fuzz_alloc_sequences.rs` and
        `tests/property_alloc_sequences.rs`.
  - [ ] Document how to minimize and commit reproducers.
  - Verify: fuzz target builds and runs for a bounded smoke duration in CI or
    release candidate checks.

- [ ] Add deterministic OOM/failure injection.
  - [ ] Identify allocator OS hooks or internal allocation paths that can be
        made fail deterministically in tests.
  - [ ] Test safe `Allocation` failure returns `AllocError` without leaking.
  - [ ] Test safe resize failure preserves the original allocation in
        `ResizeError`.
  - [ ] Test heap allocation failure preserves heap ownership and does not
        double-free.
  - [ ] Test raw allocation failure preserves C null/errno behavior where
        mimalloc specifies it.
  - [ ] Avoid relying only on huge allocation attempts, which are platform and
        overcommit sensitive.
  - Verify: focused failure-injection tests pass in debug and release.

- [ ] Add callback concurrency and reentry tests.
  - [ ] Model process-global callback registration/restoration races where
        possible.
  - [ ] Add tests for callback guard drop restoring the previous callback under
        nested registration.
  - [ ] Add tests for allocator reentry from error and deferred-free
        callbacks.
  - [ ] Add tests for panic containment in `std` and test builds.
  - [ ] Document which callback races remain caller obligations because Tier U
        constructors are unsafe.
  - Verify: callback tests and any model tests pass under the safety-stress
    script.

- [ ] Add artifact-level no-unwind tests.
  - [ ] Build the release `c-abi` cdylib with panic abort.
  - [ ] Trigger representative error, callback, and OOM paths from a C
        consumer.
  - [ ] Assert Rust panic/unwind does not cross the C ABI boundary.
  - [ ] Include callback panic containment where `std` or test builds catch
        panics.
  - Verify: no-unwind artifact test passes on Linux x86_64 GNU.

- [ ] Add sustained thread-stress coverage for release candidates.
  - [ ] Stress same-thread allocate/free.
  - [ ] Stress cross-thread free.
  - [ ] Stress abandoned page reclaim.
  - [ ] Stress thread init/done.
  - [ ] Stress process done and shutdown stats.
  - [ ] Add fork/exec constraints only if they are claimed as supported.
  - Verify: stress script is part of release-candidate validation.

## P1 - Align CI and release gates

- [ ] Make formatting a first-class validation workflow step.
  - [ ] Add `scripts/check-fmt.sh` to `.github/workflows/validation.yml`.
  - [ ] Consider changing `scripts/check-fmt.sh` to `cargo fmt --all --check`
        so it matches the project-required developer command.
  - [ ] Keep `git diff --check` for whitespace errors.
  - Verify: validation workflow runs formatting before lints.

- [ ] Add feature-sensitive ABI checks to CI.
  - [ ] Run `scripts/check-feature-abi.sh` in validation or release CI.
  - [ ] Include default, `secure`, `guarded`, `track-valgrind`, and
        `track-asan` symbol/layout checks as currently scripted.
  - [ ] Add dynamic cdylib feature-mode symbol checks after dynamic manifests
        exist.
  - Verify: feature ABI job reports each feature set separately.

- [ ] Add C and C++ artifact consumer checks to CI.
  - [ ] Run the focused C static and dynamic consumer script.
  - [ ] Run the focused `dlopen` script.
  - [ ] Run the focused `LD_PRELOAD` script.
  - [ ] Run the supported C++ override script.
  - [ ] Keep broad behavioral parity available as a release-candidate job if it
        remains too slow for every pull request.
  - Verify: validation or release workflow names the artifact mode under test.

- [ ] Align `docs/release.md` with existing hardening workflows.
  - [ ] Add ASan to the release checklist.
  - [ ] Add TSan to the release checklist.
  - [ ] Add Valgrind tracking parity to the release checklist.
  - [ ] Add Miri core unsafe validation to the release checklist.
  - [ ] Add safety-stress to the release checklist.
  - [ ] State which jobs must be green before a release tag.
  - Verify: release checklist and `.github/workflows/validation.yml` agree.

- [ ] Pin performance workflow toolchain consistently.
  - [ ] Change `.github/workflows/performance.yml` from `stable` to the pinned
        release toolchain or document why benchmark gates intentionally use
        floating stable.
  - [ ] Capture `scripts/env-info.sh` output in performance artifacts.
  - [ ] Upload benchmark CSV and environment evidence.
  - Verify: performance workflow artifacts include threshold and environment
    files.

- [ ] Add package consumer matrix coverage.
  - [ ] Keep the existing packaged safe-Rust consumer test.
  - [ ] Add a default-feature packaged consumer if it differs from the current
        `std` consumer.
  - [ ] Add an FFI artifact consumer test that does not rely on unpublished
        repo scripts.
  - [ ] Add docs.rs-style documentation build verification if not already
        covered by `scripts/check-docs.sh`.
  - Verify: `scripts/check-package.sh` plus new FFI consumer check.

## P1 - Safe API polish without broadening the API

- [ ] Add examples or doctests for every Tier S type.
  - [ ] Cover `MiMalloc`.
  - [ ] Cover `Allocation` and `InitializedAllocation`.
  - [ ] Cover `Heap`, `HeapRef`, `MainHeap`, `HeapAllocation`, and
        `InitializedHeapAllocation`.
  - [ ] Cover `GlobalOptions` and `OptionKey`.
  - [ ] Cover `StatsSnapshot`, `StatCount`, `StatTotal`, and `ProcessInfo`.
  - [ ] Cover every safe error/status type.
  - Verify: `cargo test --doc --features std`.

- [ ] Enforce rustdoc sections for safe APIs.
  - [ ] Add `# Errors` sections to fallible Tier S methods.
  - [ ] Add `# Panics` sections where a safe API can panic, or state that it
        does not panic beyond standard library argument constructors.
  - [ ] Add `# Safety` sections to all unsafe Tier U and Tier R public items.
  - [ ] Extend `scripts/check-safe-docs.py` or add a new docs check for these
        sections.
  - Verify: `scripts/check-docs.sh`.

- [ ] Enable or emulate `missing_docs` for Tier S and Tier U.
  - [ ] Decide whether to use module-level `#![warn(missing_docs)]` or extend
        the current safe-docs script.
  - [ ] Include public enum variants, type aliases, trait impl notes where
        appropriate, and unsafe callback guard docs.
  - [ ] Keep global `missing_docs` staged until raw/Tier R policy permits it.
  - Verify: docs check fails on a synthetic undocumented Tier S item.

- [ ] Add trait behavior tests for safe facade types.
  - [ ] Assert `MiMalloc` expected traits.
  - [ ] Assert `Allocation` and `InitializedAllocation` are `Send`/`Sync` only
        as documented.
  - [ ] Assert `Heap`, `HeapRef`, `MainHeap`, `HeapAllocation`, and
        `InitializedHeapAllocation` are not `Send` and not `Sync`.
  - [ ] Prefer compile-fail doctests or a dedicated static assertion helper.
  - Verify: trait tests fail if auto-traits change accidentally.

- [ ] Add compile-fail tests for heap lifetime misuse.
  - [ ] Prove a `HeapAllocation<'heap>` cannot outlive its `Heap`.
  - [ ] Prove an `InitializedHeapAllocation<'heap>` cannot outlive its `Heap`.
  - [ ] Prove deleting/dropping a heap while safe heap allocations are live is
        rejected by the borrow checker.
  - [ ] Include `MainHeap` examples separately because it produces
        `HeapRef<'static>` by design.
  - Verify: compile-fail tests run under doctest or trybuild-equivalent
    tooling.

- [ ] Add an initialization walkthrough.
  - [ ] Show `Allocation::new` returning uninitialized bytes.
  - [ ] Show writing through `MaybeUninit<u8>` without reading uninitialized
        memory.
  - [ ] Show the unsafe `assume_initialized` transition after every byte is
        initialized.
  - [ ] Show the zeroed path as the safe alternative.
  - Verify: example compiles and doctests pass.

- [ ] Keep object ownership APIs out of the current safe facade.
  - [ ] Add a release checklist item that rejects `MiBox<T>` or
        `HeapBox<'heap, T>` additions without a separate design.
  - [ ] If object APIs are designed later, cover construction failure,
        destructor behavior, panic behavior, heap ownership, and threading
        traits before implementation.
  - Verify: API inventory has no object ownership types unless the design is
    complete.

## P1 - Performance signoff

- [ ] Promote full benchmark evidence for release candidates.
  - [ ] Run `scripts/check-bench-full.sh` before release candidates.
  - [ ] Capture `scripts/env-info.sh` output.
  - [ ] Store the threshold CSV from the full run.
  - [ ] Update `BENCHMARK_RESULTS.md` with command lines, feature flags, and
        noisy-benchmark notes.
  - Verify: release checklist points to the exact benchmark evidence.

- [ ] Measure documented artifact modes separately.
  - [ ] Benchmark default Rust library mode.
  - [ ] Benchmark `interpose` mode separately.
  - [ ] Avoid reusing Criterion baselines across feature/artifact modes.
  - [ ] Record Rust-vs-C ratios for native scenario benchmarks.
  - Verify: benchmark report identifies feature set and artifact mode.

- [ ] Require evidence for new strict internal wrappers.
  - [ ] Update `docs/strict-types.md` or `docs/performance.md` to state what
        assembly or benchmark evidence is required for new hot-path witnesses.
  - [ ] Capture release assembly for free-list pop, local free, remote free,
        small page lookup, and any changed hot path.
  - [ ] Fail release signoff if a new wrapper adds branches, calls, memory
        traffic, stack spills, formatting, allocation, or panic paths in a
        tracked hot path without justification.
  - Verify: `scripts/capture-release-assembly.sh` output or benchmark evidence
    is attached to the change.

- [ ] Publish per-release performance summaries.
  - [ ] Summarize geomean Rust/C ratios.
  - [ ] Summarize hot-path deltas against previous release evidence.
  - [ ] Separate noisy watch items from release-blocking regressions.
  - [ ] Link raw Criterion summaries or CSV artifacts.
  - Verify: release notes link to `BENCHMARK_RESULTS.md` or an attached report.

## P2 - Documentation polish

- [ ] Rewrite README around three user stories.
  - [ ] Rust global allocator.
  - [ ] Safe Rust owned bytes, heaps, stats, and options.
  - [ ] C ABI artifact and interposition.
  - [ ] Keep raw API examples below the safe facade and label them unsafe and
        unstable.
  - Verify: README examples match current commands and features.

- [ ] Add a dedicated Rust safety model page.
  - [ ] Explain ownership and lifetime encoding in safe APIs.
  - [ ] Explain why uninitialized memory is not exposed as `&[u8]`.
  - [ ] Explain heap-tied allocation lifetimes.
  - [ ] Explain why process-global callback guards are Tier U unsafe APIs.
  - [ ] Explain raw pointer isolation at FFI and hot-path boundaries.
  - [ ] Explain audit tags and no-allocation critical-path evidence.
  - [ ] Explain strict internal types and zero-cost expectations.
  - Verify: README links to the page and `scripts/check-docs.sh` passes.

- [ ] Add an FFI linking guide.
  - [ ] Document dynamic C ABI linking.
  - [ ] Document static C ABI linking.
  - [ ] Document interposition with `LD_PRELOAD`.
  - [ ] Document C++ operator override modes.
  - [ ] Document headers and link metadata.
  - [ ] Document supported target and unsupported platform caveats.
  - Verify: guide commands are exercised by consumer tests.

- [ ] Add a symbol and versioning policy page or section.
  - [ ] Define public C header symbols.
  - [ ] Define malloc-family interposition symbols.
  - [ ] Define C++ operator symbols.
  - [ ] Define extension ABI naming rules.
  - [ ] Define static archive internal-symbol policy.
  - [ ] Define release-to-release ABI diff expectations.
  - Verify: policy matches symbol scripts and manifests.

- [ ] Add a migration guide from C mimalloc.
  - [ ] Map C headers to Rust release artifacts.
  - [ ] Map common allocation/free APIs to safe Rust and raw Rust choices.
  - [ ] Explain callback differences and Tier U guard obligations.
  - [ ] Explain options/stats safe wrappers.
  - [ ] Explain unsupported platform and feature differences.
  - Verify: guide links to compatibility and ABI docs.

- [ ] Update production status text.
  - [ ] State exactly what is supported.
  - [ ] State exactly what is preview-only.
  - [ ] State exactly what is unstable.
  - [ ] Keep Linux x86_64 GNU as the only maintained target until more evidence
        exists.
  - Verify: README, `docs/limitations.md`, `docs/platforms.md`, and
    `docs/release.md` agree.

## P2 - Platform promotion policy

- [ ] Convert platform promotion rules into a checklist.
  - [ ] Require compile and test CI for the platform.
  - [ ] Require ABI/layout checks where the platform has a C ABI story.
  - [ ] Require C and C++ link tests where FFI artifacts are claimed.
  - [ ] Require sanitizer or equivalent hardening evidence.
  - [ ] Require platform primitive audit.
  - [ ] Require benchmark evidence or an explicit performance caveat.
  - [ ] Require documented limitations.
  - Verify: `docs/platforms.md` contains the checklist.

- [ ] Keep source-present platforms out of release marketing.
  - [ ] Ensure macOS, Windows, WASI, Emscripten, aarch64, and other source
        present targets are described as smoke/source-present only unless
        promoted.
  - [ ] Ensure CI smoke jobs do not imply maintained support.
  - [ ] Add release-note language for unsupported target reports and triage.
  - Verify: README and platform docs do not overclaim.

## P2 - Security and governance

- [ ] Add a concrete private vulnerability reporting process.
  - [ ] Add a security email address or GitHub Security Advisory instructions.
  - [ ] State expected acknowledgement and triage timelines.
  - [ ] State what information reporters should include.
  - [ ] Keep unsupported-target triage policy clear.
  - Verify: `SECURITY.md` has a concrete private contact path.

- [ ] Add release signing.
  - [ ] Choose signing mechanism for source tags and binary artifacts.
  - [ ] Document key ownership and rotation.
  - [ ] Sign release checksums or artifacts.
  - [ ] Verify signatures in the release dry run.
  - Verify: release checklist includes signing verification.

- [ ] Add SBOM or dependency report.
  - [ ] Generate a dependency report for runtime and development dependencies.
  - [ ] Include build-tool requirements such as C compiler, C++ compiler, and
        archiver.
  - [ ] Attach the report to release artifacts or release notes.
  - Verify: release dry run includes the SBOM/dependency report.

- [ ] Expand reproducible release instructions.
  - [ ] Document clean checkout requirements.
  - [ ] Document exact Rust toolchain and components.
  - [ ] Document artifact build commands.
  - [ ] Document expected checksums and symbol manifests.
  - [ ] Document environment capture.
  - Verify: another checkout can reproduce artifact names and manifests.

- [ ] Add supported-version policy after releases exist.
  - [ ] Define which release lines receive security fixes.
  - [ ] Define patch release expectations.
  - [ ] Define when pre-release branches stop receiving fixes.
  - Verify: `SECURITY.md` and release docs agree.

- [ ] Add CVE and advisory handling policy.
  - [ ] Define severity triage.
  - [ ] Define embargo handling.
  - [ ] Define coordinated disclosure steps.
  - [ ] Define changelog and advisory publication requirements.
  - Verify: governance docs link to the policy.

- [ ] Add fuzz corpus and reproducer policy.
  - [ ] Define where minimized reproducers live.
  - [ ] Define when corpus additions are required.
  - [ ] Define how to classify invalid-caller UB versus allocator bugs.
  - [ ] Define how fuzz findings map to unsafe audit entries.
  - Verify: fuzzing docs and security docs agree.

- [ ] Add a hardening threat model.
  - [ ] Describe what `secure` is expected to detect or mitigate.
  - [ ] Describe what `guarded` is expected to detect or mitigate.
  - [ ] Describe what remains out of scope for malicious unsafe callers.
  - [ ] Describe entropy, canary, and metadata corruption assumptions.
  - Verify: `SECURITY.md` links to the threat model.

- [ ] Add memory corruption triage checklist.
  - [ ] Include allocation/free/realloc reproduction data.
  - [ ] Include feature set and target platform.
  - [ ] Include sanitizer/Valgrind/Miri evidence where applicable.
  - [ ] Include upstream C mimalloc comparison requirement.
  - [ ] Include symbol/artifact mode if the report involves FFI.
  - Verify: issue template or security docs include the checklist.

- [ ] Define upstream sync cadence.
  - [ ] State how often vendored mimalloc is reviewed.
  - [ ] State what must be re-run when upstream headers or implementation
        change.
  - [ ] State how `UPSTREAM_DIFF.md`, `PARITY.md`, ABI manifests, and
        benchmark evidence are updated.
  - Verify: `docs/upstream.md` includes cadence and required checks.

## Definition of done for a production-style release

- [ ] Tier S/Tier U Rust API contract is frozen and diffed.
- [x] `Options` alias decision is complete.
- [x] `raw` release policy is explicit and tested.
- [ ] Dynamic C ABI has no unintentional extra `mi_*` exports.
- [ ] Staticlib is either clean enough for production or explicitly
      preview-only.
- [ ] Release artifacts include headers, link metadata, checksums, symbol
      manifests, and reproducibility notes.
- [ ] C and C++ consumers link dynamically and statically in CI or release CI.
- [ ] `LD_PRELOAD` interposition is tested for the documented artifact mode.
- [ ] ASan, TSan, Valgrind, Miri, layout, symbol, and parity checks are release
      gates.
- [ ] Maintained external fuzz targets exist and have seed corpora.
- [ ] Every unsafe audit tag maps to tests, tooling evidence, or manual proof.
- [ ] Benchmark report shows wrapper and strict-type changes stay within
      documented budgets.
- [ ] Docs clearly distinguish safe Rust API, unsafe raw Rust API, and C ABI
      artifacts.
- [ ] Supported target claims remain narrow and evidence-backed.
- [ ] Security reporting, signing, SBOM/dependency report, and release
      reproduction process are documented.
