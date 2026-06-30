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
- Translated implementation modules are private and no longer exposed through
  the raw facade.
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

- [x] Add a release API diff artifact for Tier S and Tier U.
  - [x] Choose either an existing tool such as `cargo public-api` or extend
        `scripts/check-api-inventory.py` to emit a checked snapshot.
  - [x] Store the release snapshot under a stable path such as
        `docs/api-snapshots/`.
  - [x] Add release instructions for comparing the current snapshot to the
        previous release tag.
  - [x] Ensure raw/Tier R changes remain classified separately from Tier S/U
        Rust source compatibility.
  - Evidence: `scripts/check-api-inventory.py` now checks
    `docs/api-snapshots/tier-s-u.txt`, can regenerate it with
    `--write-tier-su-snapshot`, and `docs/release.md` documents previous-tag
    diff review while excluding Tier R/Tier C from the snapshot.
  - Verify: release snapshot command exits cleanly and the snapshot diff is
    reviewed.

## P0 - Clean the dynamic C ABI

- [x] Replace the broad Linux `mi_*` dynamic export pattern with an exact
      allowlist.
  - [x] Generate the public `mi_*` allowlist from
        `mimalloc-3.3.2/include/mimalloc.h` and
        `mimalloc-3.3.2/include/mimalloc-stats.h`.
  - [x] Include only documented public-header symbols in the default C ABI
        symbol group.
  - [x] Keep Rust-extension `mi_*` helpers out of the default dynamic export
        set unless they are renamed or documented as extension ABI.
  - [x] Fail the check if a new dynamic `mi_*` symbol appears without an
        explicit public-header or extension-ABI decision.
  - Evidence: `build.rs` generates the Linux cdylib `mi_*` version-script
    allowlist from the vendored public headers, extension-only `mi_*` helpers
    are not `no_mangle` in `c-abi` builds, and
    `scripts/check-public-symbols.py --dynamic-mi` fails on any dynamic
    `mi_*` export outside the public headers.
  - Verify: new dynamic symbol check plus `scripts/check-abi-symbols.sh`.

- [x] Split dynamic export groups by artifact mode.
  - [x] For `--features c-abi --crate-type cdylib`, export public mimalloc
        `mi_*` symbols and required public layouts/callbacks only.
  - [x] For `--features interpose --crate-type cdylib`, add malloc-family
        override symbols.
  - [x] For `--features cxx-overrides` or `interpose`, add Itanium C++
        operator new/delete override symbols only when the C++ shim is linked.
  - [x] Ensure `c-abi` alone does not export malloc-family or C++ operator
        symbols by version-script pattern.
  - [x] Add negative tests proving those symbols are absent from the wrong
        artifact modes.
  - Evidence: `build.rs` now appends C++ and malloc-family export groups only
    for the matching artifact modes, `src/raw/macros.rs` gates the corresponding
    `export_name` attributes in `c-abi` builds, and
    `scripts/check-public-symbols.py --dynamic-groups` verifies the negative
    and positive dynamic export cases for `c-abi`, `cxx-overrides`, and
    `interpose`.
  - Verify: `readelf -Ws` or `nm -D` checks for `c-abi`, `cxx-overrides`, and
    `interpose` cdylibs.

- [x] Decide the Rust-extension symbol policy.
  - [x] Review each `rust-extension` and `platform-helper-extension` entry in
        `symbols/public-symbol-classification.tsv`.
  - [x] For each extra `mi_*` symbol, choose one outcome: hide, rename with a
        `mechanical_mimalloc_*` prefix, or document as intentional extension
        ABI.
  - [x] If hidden, remove it from the dynamic export allowlist and keep only
        internal Rust reachability.
  - [x] If renamed, provide compatibility only if the old symbol was already
        part of a promised release.
        Not applicable: no current Rust-extension `mi_*` symbol was renamed for
        this release.
  - [x] If documented, add ownership, nullability, threading, and stability
        text to `docs/abi.md`.
        Not applicable: no current Rust-extension `mi_*` symbol is promoted to
        intentional extension ABI for this release.
  - Evidence: `docs/abi.md`, `docs/api-inventory.md`, and
    `symbols/public-symbol-classification.tsv` now state that current
    `rust-extension` and `platform-helper-extension` `mi_*` symbols are hidden
    from default dynamic C ABI artifacts and remain only static/no-`c-abi`
    compatibility classifications unless a future release explicitly promotes
    one as extension ABI.
  - Verify: dynamic symbol manifest shows no unintentional extra `mi_*`
    symbols.

- [x] Add dynamic ABI golden manifests.
  - [x] Produce one checked manifest for `c-abi` cdylib exports.
  - [x] Produce one checked manifest for `interpose` cdylib exports.
  - [x] Include symbol names, binding, visibility, version where available, and
        artifact build command.
  - [x] Add a script that diffs current output against the checked manifest.
  - [x] Document how to intentionally update the manifest.
  - Evidence: `docs/abi-manifests/c-abi-dynamic.tsv` and
    `docs/abi-manifests/interpose-dynamic.tsv` are checked manifests,
    `scripts/check-public-symbols.py --dynamic-manifests` diffs current
    `readelf --dyn-syms` output against them,
    `scripts/check-public-symbols.py --dynamic-manifest-self-test` verifies an
    injected extra symbol is detected, and `docs/abi.md`/`docs/release.md`
    document the update command.
  - Verify: manifest check fails on an injected extra symbol in a local
    self-test or fixture.

- [x] Add ABI diff checks between releases.
  - [x] Define where previous release symbol manifests are stored or fetched.
  - [x] Compare new cdylib and staticlib manifests against the previous release.
  - [x] Classify removed, added, renamed, and visibility-changed symbols.
  - [x] Require a changelog entry for every intentional ABI change.
  - Evidence: `scripts/check-abi-diff.py` compares the current checked
    `docs/abi-manifests/*.tsv` files against either `--previous-ref` or
    `--previous-dir`, so current cdylib manifests and future staticlib
    manifests share one release-diff path. It classifies added, removed,
    possible-renamed, visibility-changed, binding-changed, version-changed, and
    type-changed symbols, requires `ABI-CHANGE: <manifest> <symbol>` changelog
    markers for intentional differences, and has a self-test wired into
    `scripts/check-abi-symbols.sh`.
  - Verify: release checklist includes the ABI diff command.

## P0 - Decide and clean the staticlib story

- [x] Decide whether staticlib is production-supported or preview-only.
  - [x] If production-supported, define the exact static archive symbol
        contract.
        Not applicable: the current release decision is preview-only.
  - [x] If preview-only, state this in README, `docs/abi.md`,
        `docs/release.md`, and the FFI linking guide.
  - [x] Make the decision visible in release notes and artifact names.
  - Evidence: README, `docs/abi.md`, `docs/ffi-linking.md`, `docs/release.md`,
    `docs/compatibility.md`, and `CHANGELOG.md` now state that `staticlib` is
    preview-only. The release policy requires preview-marked artifact names
    such as `mechanical_mimalloc-static-preview` until the static archive
    symbol contract, collision tests, and checked static manifests are
    completed.
  - Verify: docs and release checklist use the same staticlib status.

- [x] If staticlib is production-supported, reduce accidental symbol leakage.
  - [x] Audit every `_mi_*` static archive export currently covered by the
        prefix classification.
        Not applicable for this release: staticlib is preview-only.
  - [x] Determine which internal C-shaped symbols are truly required by the
        static archive link model.
        Not applicable for this release: no production static archive symbol
        contract is frozen.
  - [x] Hide, make Rust-private, or prefix internal symbols that do not need
        external linkage.
        Not applicable for this release: static archive exports remain
        preview-only and explicitly unstable.
  - [x] Add a static archive symbol manifest with expected public and internal
        groups.
        Not applicable for this release: checked static manifests are required
        before changing the staticlib policy to production-supported.
  - [x] Fail CI if a new `_mi_*`, `mi_*`, malloc-family, or C++ symbol appears
        without classification.
        Existing static symbol classification remains checked by
        `scripts/check-public-symbols.py`.
  - Evidence: staticlib is preview-only in README, `docs/abi.md`,
    `docs/ffi-linking.md`, `docs/release.md`, and `docs/compatibility.md`.
    Production static archive leakage reduction and checked static manifests
    are deferred until the release policy changes from preview-only to
    production-supported; the current static archive classification gate remains
    `scripts/check-public-symbols.py`.
  - Verify: `scripts/check-public-symbols.py` plus the new static manifest
    check.
    Static manifest check not applicable until staticlib is
    production-supported.

- [x] Add static link collision tests.
  - [x] Build a C app that links the Rust staticlib and another object/library
        with representative non-conflicting symbols.
  - [x] Add a negative fixture for a known collision class if static leakage is
        intentionally preview-only.
  - [x] Verify malloc-family symbols behave as documented in static link mode.
  - [x] Verify internal `_mi_*` symbols are either absent or explicitly
        documented as not stable.
  - Evidence: `scripts/check-static-link-collisions.py` builds and runs a C
    program linked against the Rust staticlib plus an unrelated support object,
    verifies `malloc` allocations are owned by mimalloc in that static link
    mode, checks the preview-only internal `_mi_*` documentation, and asserts a
    duplicate `_mi_tls_theap_default_get` object fails to link as the documented
    preview collision class.
  - Verify: new static collision script in the release checklist.

## P0 - Build and ship release artifacts

- [x] Choose the FFI artifact distribution model.
  - [x] Decide between crates.io Rust crate only plus GitHub release assets,
        crates.io package with enough source-build instructions, or a split
        Rust/FFI crate model.
  - [x] Document why the chosen model works even though `scripts/` and
        vendored headers are not included in the current crate package.
  - [x] Update `docs/release.md` and README with the chosen model.
  - Evidence: README, `docs/release.md`, and `docs/ffi-linking.md` now define
    the model as a crates.io Rust library package plus GitHub release assets
    for C ABI libraries, headers, link metadata, checksums, ABI manifests, and
    reproducibility notes. The docs explicitly say FFI assets are built from
    the tagged repository checkout because repo-only scripts and vendored C
    headers are intentionally outside the crate package, and that a split
    Rust/FFI crate is deferred until there is a separate FFI package contract.
  - Verify: `scripts/check-package.sh` and a release dry-run artifact build.

- [x] Add an explicit release artifact build script.
  - [x] Build the Rust library artifact:
        `cargo build --release --lib`.
  - [x] Build C ABI cdylib:
        `cargo rustc --release --features c-abi --lib --
        --crate-type cdylib`.
  - [x] Build C ABI staticlib:
        `cargo rustc --release --features c-abi --lib --
        --crate-type staticlib`.
  - [x] Build interpose cdylib:
        `cargo rustc --release --features interpose --lib --
        --crate-type cdylib`.
  - [x] Build interpose staticlib:
        `cargo rustc --release --features interpose --lib --
        --crate-type staticlib`.
  - [x] Use isolated `CARGO_TARGET_DIR` values so artifacts from different
        modes cannot be confused.
  - [x] Emit a machine-readable manifest containing artifact path, feature set,
        target triple, profile, rustc version, and checksum.
  - Evidence: `scripts/build-release-artifacts.py` builds the Rust `rlib`,
    `c-abi` cdylib, `c-abi` staticlib preview, `interpose` cdylib, and
    `interpose` staticlib preview with separate `CARGO_TARGET_DIR` values,
    copies outputs under `target/release-artifacts/artifacts/`, and writes
    `target/release-artifacts/release-artifacts.json` with artifact path,
    feature set, crate type, target triple, release profile, rustc version,
    build command, and SHA-256 checksum.
  - Verify: artifact build script exits cleanly from a fresh checkout.

- [x] Ship C headers with release artifacts.
  - [x] Include `mimalloc.h`.
  - [x] Include `mimalloc-stats.h`.
  - [x] Decide whether to include `mimalloc-override.h` for interposition
        users.
        Decision: include it with release artifacts for C interposition users.
  - [x] Decide whether to include `mimalloc-new-delete.h` for C++ users.
        Decision: include it with release artifacts for C++ override users.
  - [x] Record the header hashes from `PARITY.md` or regenerate them in the
        artifact manifest.
  - Evidence: `scripts/build-release-artifacts.py` stages `mimalloc.h`,
    `mimalloc-stats.h`, `mimalloc-override.h`, and
    `mimalloc-new-delete.h` under `target/release-artifacts/include/` and
    records each header path, source path, byte length, role, and SHA-256 in
    `target/release-artifacts/release-artifacts.json`.
    `scripts/check-release-artifact-consumer.py` builds and runs a C consumer
    with only the staged include directory and staged `c-abi` cdylib.
  - Verify: a C consumer builds using only the staged release artifact
    directory, not the repository source tree.

- [x] Add link metadata for FFI consumers.
  - [x] Generate `mechanical_mimalloc.pc`.
  - [x] Generate `mechanical_mimalloc-config.cmake`.
  - [x] Include library search paths, include paths, pthread/dl/m link
        requirements, and artifact-mode notes.
  - [x] Test pkg-config with a C consumer.
  - [x] Test CMake `find_package` with a C consumer.
  - Evidence: `scripts/build-release-artifacts.py` writes
    `target/release-artifacts/lib/pkgconfig/mechanical_mimalloc.pc` and
    `target/release-artifacts/lib/cmake/mechanical_mimalloc/mechanical_mimalloc-config.cmake`,
    records both files with SHA-256 checksums in `link_metadata`, and keeps the
    default package link target on the production `c-abi` cdylib while exposing
    preview static artifacts as explicitly named preview variables/targets.
    `scripts/check-release-link-metadata.py` copies the staged release tree to a
    temporary directory outside the repository, then builds and runs C consumers
    through pkg-config and CMake `find_package`.
  - Verify: package metadata tests run from a temporary directory outside the
    repository.

- [x] Produce release integrity files.
  - [x] Generate `SHA256SUMS`.
  - [x] Generate a checked ABI symbol manifest for every shipped binary
        artifact.
  - [x] Generate an environment/reproducibility note using
        `scripts/env-info.sh`.
  - [x] Include benchmark report references or attach the benchmark summary
        required by `docs/performance.md`.
  - Evidence: `scripts/build-release-artifacts.py` writes `SHA256SUMS`,
    checked ABI symbol manifests under
    `target/release-artifacts/integrity/abi-symbols/`, an
    `environment.txt` capture from `scripts/env-info.sh`, and
    `benchmark-evidence.md` with `docs/performance.md`,
    `BENCHMARK_RESULTS.md`, and benchmark command references.
    `scripts/check-release-integrity.py` performs a fresh dry run and validates
    the checksum file plus regenerated cdylib/staticlib symbol manifests.
  - Verify: release dry-run produces all expected files.

- [x] Fix stale C API build instructions.
  - [x] Update `examples/c_api.c`; `cargo build --release` no longer builds a
        linkable cdylib with the current `[lib] crate-type = ["rlib"]`.
  - [x] Show the correct cdylib and staticlib commands.
  - [x] Show the correct `cc` command for dynamic linking.
  - [x] Show the correct `cc` command for static linking.
  - [x] Keep the example aligned with the chosen artifact distribution model.
  - Evidence: `examples/c_api.c` now states that default release builds are
    Rust `rlib` builds, shows explicit `cargo rustc --features c-abi`
    `cdylib` and `staticlib` artifact commands, links the dynamic example
    directly against the produced cdylib with an rpath, links the static
    preview example directly against a native-object archive built with
    explicit no-LTO/no-bitcode `RUSTFLAGS` plus `pthread`, `dl`, and `m`, and
    points release users at
    `scripts/build-release-artifacts.py`.
  - Verify: run the documented commands from a clean target directory.

## P0 - Add real C and C++ consumer tests

- [x] Extend C public API link tests to cover release artifact modes.
  - [x] Keep the existing static public API smoke coverage.
  - [x] Add a dynamic C smoke test against the `c-abi` cdylib.
  - [x] Add a static C smoke test against the `c-abi` staticlib.
  - [x] Add a dynamic C smoke test against the `interpose` cdylib when
        interposition symbols are expected.
  - [x] Add a static C smoke test against the `interpose` staticlib.
  - [x] Build consumers from a temporary directory using staged headers and
        libraries.
  - Evidence: `scripts/check-release-artifact-consumer.py` now rebuilds the
    staged release artifacts, copies them to a temporary directory outside the
    repository, verifies staged headers, and builds/runs C consumers against
    the `c-abi` cdylib, `c-abi` static preview archive, `interpose` cdylib, and
    `interpose` static preview archive. `scripts/build-release-artifacts.py`
    records static preview `rustflags` and builds native-object static archives
    so the static C smoke tests use the platform C linker.
  - Verify: new C consumer script is run by validation or release CI.

- [x] Add focused `dlopen`/`dlsym` tests.
  - [x] Build the documented `c-abi` cdylib.
  - [x] `dlopen` the artifact with `RTLD_NOW`.
  - [x] Resolve `mi_malloc`, `mi_free`, `mi_good_size`, and representative
        stats/options symbols.
  - [x] Allocate and free through resolved function pointers.
  - [x] Assert plain `malloc` is not redirected by `dlopen` alone.
  - [x] Assert no unexpected extension symbol is resolvable in the clean
        default C ABI artifact.
  - Evidence: `scripts/check-release-dlopen.py` rebuilds staged release
    artifacts, copies the release payload to a temporary directory outside the
    repository, compiles a C probe, `dlopen`s the staged `c-abi` cdylib with
    `RTLD_NOW | RTLD_LOCAL`, resolves `mi_malloc`, `mi_free`, `mi_good_size`,
    `mi_stats_reset`, `mi_option_get`, and `mi_check_owned`, allocates/frees via
    resolved pointers, verifies plain libc `malloc` is not owned after `dlopen`
    alone, and verifies `mi_heap_check_owned` is absent from the clean C ABI
    artifact.
  - Verify: focused dynamic loading script passes on Linux x86_64 GNU.

- [x] Add focused `LD_PRELOAD` interposition tests.
  - [x] Build the documented `interpose` cdylib.
  - [x] Run a C program under `LD_PRELOAD` and verify `malloc`, `calloc`,
        `realloc`, `free`, `aligned_alloc`, `posix_memalign`, `memalign`,
        `valloc`, and `malloc_usable_size` route to the allocator.
  - [x] Verify `mi_check_owned` or equivalent ownership checks where the C
        contract supports them.
  - [x] Verify shutdown stats behavior when `mimalloc_show_stats=1`.
  - [x] Keep this as a small CI-friendly script even if
        `scripts/check-behavioral-parity.py` remains broader.
  - Evidence: `scripts/check-release-ld-preload.py` rebuilds the staged
    release artifacts, copies the release payload to a temporary directory
    outside the repository, compiles a C probe, runs it with the staged
    `interpose` cdylib in `LD_PRELOAD`, verifies malloc-family allocations are
    owned by mimalloc through `mi_check_owned`, checks `malloc_usable_size` and
    alignment-sensitive allocation APIs, and runs a stats-enabled pass that
    verifies shutdown stats sections are emitted for `mimalloc_show_stats=1`.
  - Verify: focused LD_PRELOAD script passes on Linux x86_64 GNU.

- [x] Align broad behavioral parity with documented artifact modes.
  - [x] Update `scripts/check-behavioral-parity.py` so `rust_cdylib()` builds
        the documented dynamic artifact mode, not an unfeatured cdylib unless
        that is intentionally being tested.
  - [x] Add separate helper names for plain cdylib, `c-abi` cdylib, and
        `interpose` cdylib if all three modes remain useful.
        Plain unfeatured cdylib is not used by the supported parity matrix.
  - [x] Record which parity cases require interposition and which require only
        C ABI exports.
  - [x] Add command-line flags if the full parity script remains too expensive
        for every CI run.
  - Evidence: `scripts/check-behavioral-parity.py` now has explicit
    `rust_c_abi_cdylib()` and `rust_interpose_cdylib()` helpers with isolated
    Cargo target directories. Dynamic `dlopen` parity loads the Rust `c-abi`
    cdylib, while dynamic `LD_PRELOAD` and show-stats parity use the Rust
    `interpose` cdylib. The script prints the artifact mode plan at startup,
    supports `--list-artifact-modes`, and supports `--only all|core|dynamic`
    so CI can run just the dynamic artifact-mode slice. The release-static
    helper now uses native-object rustflags so its C linker smoke tests do not
    depend on LLVM bitcode support.
  - Verify: `scripts/check-behavioral-parity.py --list-artifact-modes` and
    `scripts/check-behavioral-parity.py --only dynamic` pass and identify the
    exact artifact modes under test.

- [x] Add C++ release artifact tests.
  - [x] Test C++ operator new/delete overrides for the `interpose` staticlib.
  - [x] Test C++ operator new/delete overrides for the `interpose` cdylib if
        that is a supported artifact.
  - [x] Test nothrow, aligned, sized delete, array new/delete, and
        `std::new_handler` behavior.
  - [x] Keep the documented bad-allocation divergence for Rust-originated
        `mi_new*` C ABI entry points.
  - Evidence: `scripts/check-release-cxx-artifacts.py` rebuilds staged release
    artifacts, copies the release payload outside the repository, compiles a
    C++20 probe against the staged `interpose` cdylib and `interpose` static
    preview archive, and verifies scalar, array, nothrow, sized delete,
    aligned object, aligned raw, aligned nothrow, and `std::new_handler`
    throwing behavior. `build.rs` now exposes `mi_cxx_shim_linked`, and
    `src/raw/macros.rs` suppresses Rust fallback `operator new` exports when
    the C++ shim is linked so release artifacts use the throwing C++ shim while
    retaining the documented Rust-originated `mi_new*` C ABI divergence.
    `scripts/check-cxx-operator-new-parity.py` now builds its Rust staticlib
    with `--features cxx-overrides`, matching the behavior it verifies.
  - Verify: `scripts/check-release-cxx-artifacts.py` passes for supported C++
    release artifact modes.

## P0 - Reduce public macro surface

- [x] Decide which exported macros are part of the production contract.
  - [x] Review the `tier-r-typed-allocation` macro list.
  - [x] Review the `tier-r-header-override` macro list.
  - [x] Review every `internal-compatibility` macro.
  - [x] Choose keep, feature-gate, make crate-private, or remove for each
        macro bucket before a stable release.
  - [x] Document the decision in `docs/api-inventory.md` and `docs/abi.md`.
  - Evidence: `docs/api-inventory.md` now records the macro production
    contract: keep typed allocation macros as Tier R C-header compatibility,
    keep header override macros as Tier R C/C++ header compatibility until the
    separate feature-gating decision, and treat `internal-compatibility` macros
    as temporary exports to privatize, remove, or explicitly retain with
    downstream expansion, translated parity, or hot-path evidence before a
    stable release. `docs/abi.md` mirrors that ABI policy.
  - Verify: `scripts/check-api-inventory.py`.

- [x] Remove or privatize internal compatibility macros where possible.
  - [x] Inventory each internal macro call site.
  - [x] Replace single-crate call sites with crate-private `macro_rules!`
        definitions when downstream expansion does not require
        `#[macro_export]`.
  - [x] Replace simple statement-like macros with functions where that does
        not affect hot-path codegen or translated parity.
  - [x] Keep `#[macro_export]` only where downstream expansion or C-header
        parity requires it.
  - [x] Add release-fast-path or assembly evidence for any hot-path macro
        replacement.
  - Evidence: unused `MI_FORWARD1!`, `MI_FORWARD3!`, `MI_FORWARD02!`, and
    `MI_INTERPOSE_MI!` exports were removed; `MI_INTERPOSE_FUN!` is now a
    file-local raw interposition helper; stats declaration macros were removed
    from the Rust macro surface, with internal stats reporting using a private
    initializer function instead. No allocator hot-path macro body was moved or
    replaced; the only function replacement is in stats print/JSON setup.
  - Verify: `cargo test --workspace --all-targets --all-features` and relevant
    benchmark smoke if hot paths moved.

- [x] Feature-gate C-header compatibility macros if they are not part of the
      default Rust contract.
  - [x] Decide on a feature name such as `c-header-macros` if gating is chosen.
  - [x] Ensure typed allocation macros and header override macros are available
        only in intended configurations.
  - [x] Update examples and doctests to enable the feature when needed.
  - [x] Update `scripts/check-api-inventory.py` to check macro inventory per
        feature set.
  - [x] Keep compile-fail tests proving raw macros require caller-side unsafe
        blocks.
  - Evidence: added the `c-header-macros` feature; gated typed allocation
    macros in `src/ffi/abi.rs` and header override macros in
    `src/ffi/headers.rs`; taught `scripts/check-api-inventory.py` to verify
    the default macro surface separately from the feature-expanded surface; and
    added downstream macro tests for feature-off absence, feature-on expansion,
    and caller-side unsafe enforcement.
  - Verify: `cargo test --doc --features std` plus macro feature checks.

## P1 - Retire `unstable-raw-internals`

- [x] Move internal-invariant integration tests into crate-private unit tests
      where practical.
  - [x] Start with tests listed in `docs/raw-facade-migration.md` that use
        `raw::internal`, `raw::types`, `raw::page`, `raw::bits`, `raw::track`,
        or `raw::bitmap`.
  - [x] Keep public C API behavior tests on root `raw::mi_*` paths.
  - [x] Avoid exposing new public helper APIs just to satisfy tests.
  - Evidence: bitmap, free-list encoding, secure constants, tracking
    constants, page bin characterization, and `_mi_assert_fail` abort coverage
    moved into crate-private unit tests. `tests/original_main_override_static.rs`,
    `tests/original_test_api_fill.rs`, and `tests/ffi_panic_audit.rs` no longer
    require private raw implementation module imports; the remaining
    source-shape checks in `tests/release_fast_path_audit.rs` read source files
    instead of importing private raw modules.
  - Verify: `scripts/check-raw-facade.sh`.

- [x] Remove the `unstable-raw-internals` feature after no repository tests
      require it.
  - [x] Delete gated module re-exports from `src/raw/mod.rs`.
  - [x] Remove feature references from `Cargo.toml`.
  - [x] Update `docs/raw-facade-migration.md`, `docs/api-inventory.md`,
        `docs/features.md`, and `docs/limitations.md`.
  - [x] Update scripts that currently expect unstable raw-internals modules.
  - Evidence: `unstable-raw-internals` was removed from `Cargo.toml`; raw
    implementation modules were removed from `src/raw/mod.rs`; release audit
    layout checks moved into crate-private unit tests; docs now describe
    implementation modules as private; and validation scripts reject raw
    implementation-module imports instead of looking for an unstable gate.
  - Verify: `scripts/check-api-inventory.py`, `scripts/check-raw-facade.sh`,
    and the full Rust test suite.

## P1 - Finish unsafe audit coverage

- [x] Require `SAFETY[AUDIT:*]` markers for every unsafe operation.
  - [x] Expand `scripts/check-unsafe-audit.py` beyond the currently migrated
        file subset.
  - [x] Detect untagged `unsafe { ... }` blocks.
  - [x] Detect untagged `unsafe impl` blocks.
  - [x] Detect untagged `unsafe extern` blocks and exported ABI wrappers.
  - [x] Detect untagged naked/assembly or export-name safety boundaries where
        the source syntax permits it.
  - [x] Provide a migration allowlist only for files with explicit owner and
        removal date.
  - Evidence: `scripts/check-unsafe-audit.py` now scans every Rust source file
    under `src/` for untagged unsafe blocks, impls, extern blocks, unsafe
    attributes, and assembly macro boundaries. Legacy untagged files must be
    listed in `docs/unsafe-migration-allowlist.tsv` with owner, removal date,
    and reason; stale or malformed entries fail the check.
  - Verify: `scripts/check-unsafe-audit.py`.

- [x] Map every audit tag to tests or manual evidence.
  - [x] Add a table to `UNSAFE_AUDIT.md` or a machine-readable audit evidence
        file.
  - [x] For each `SAFETY[AUDIT:*]` tag, list direct tests, parity scripts,
        Miri model tests, sanitizer evidence, benchmark/assembly evidence, or
        manual proof.
  - [x] Add a script that fails when a source audit tag has no evidence entry.
  - [x] Add a script that fails when an evidence entry references a missing
        test or script.
  - Evidence: `docs/unsafe-audit-evidence.tsv` maps every source
    `SAFETY[AUDIT:*]` operation ID to existing tests, scripts, or structural
    audit checks. `scripts/check-unsafe-audit-evidence.py` rejects missing
    source IDs, unknown IDs, malformed evidence rows, and evidence paths that do
    not exist; CI, release docs, and `scripts/check-local.sh` now run it.
  - Verify: new audit-evidence check plus `scripts/check-unsafe-audit.py`.

- [x] Add an advisory unsafe inventory report.
  - [x] Choose `cargo geiger` or a local equivalent.
  - [x] Generate a report for normal, `c-abi`, `interpose`, `secure`,
        `guarded`, `track-valgrind`, and `track-asan` builds.
  - [x] Store the release report as advisory evidence, not as the sole safety
        proof.
  - [x] Document how new unsafe count changes are reviewed.
  - Evidence: `scripts/check-unsafe-inventory.py` provides the local equivalent
    to `cargo geiger`, generating `docs/unsafe-inventory.md` with advisory
    unsafe counts and the required build-mode commands. The report states that
    it is not the sole safety proof and documents the review policy for count
    changes. CI and `scripts/check-local.sh` reject stale reports; the release
    checklist runs the script with `--verify-builds`.
  - Verify: release checklist includes the unsafe inventory report command.

## P1 - Add real fuzzing and failure injection

- [x] Add maintained external fuzz targets.
  - [x] Create a `cargo-fuzz` workspace or an equivalent maintained fuzzing
        harness.
  - [x] Fuzz raw allocation operation sequences: malloc, zalloc, calloc,
        realloc, aligned allocation, heap allocation, collect, and free.
  - [x] Include nulls, zero sizes, overflow-sized requests, small sizes, large
        sizes, and alignment classes.
  - [x] Include per-heap ownership, cross-thread free, realloc preservation,
        zeroing, usable-size lower bounds, and stats consistency.
  - [x] Add corpus seeds derived from `tests/fuzz_alloc_sequences.rs` and
        `tests/property_alloc_sequences.rs`.
  - [x] Document how to minimize and commit reproducers.
  - Evidence: `fuzz/` now contains an external allocation-sequence fuzz harness
    with corpus seeds and deterministic generated inputs. `scripts/check-fuzz-smoke.sh`
    builds and runs the target for a bounded smoke duration, and
    `scripts/check-safety-stress.sh` runs that smoke target with the other
    safety-stress checks. `docs/testing.md` documents deeper runs, minimization,
    corpus commits, and invalid-caller classification.
  - Verify: fuzz target builds and runs for a bounded smoke duration in CI or
    release candidate checks.

- [x] Add deterministic OOM/failure injection.
  - [x] Identify allocator OS hooks or internal allocation paths that can be
        made fail deterministically in tests.
  - [x] Test safe `Allocation` failure returns `AllocError` without leaking.
  - [x] Test safe resize failure preserves the original allocation in
        `ResizeError`.
  - [x] Test heap allocation failure preserves heap ownership and does not
        double-free.
  - [x] Test raw allocation failure preserves C null/errno behavior where
        mimalloc specifies it.
  - [x] Avoid relying only on huge allocation attempts, which are platform and
        overcommit sensitive.
  - Evidence: safe wrapper allocation and resize paths now have a `#[cfg(test)]`
    deterministic failure hook that never enters production builds. Unit tests
    cover `Allocation` returning `AllocError`, resize returning `ResizeError`
    with the original allocation and bytes preserved, heap allocation/resize
    failure leaving the heap usable, and deterministic raw product-overflow
    null returns without relying on huge overcommit-sensitive requests.
  - Verify: focused failure-injection tests pass in debug and release.

- [x] Add callback concurrency and reentry tests.
  - [x] Model process-global callback registration/restoration races where
        possible.
  - [x] Add tests for callback guard drop restoring the previous callback under
        nested registration.
  - [x] Add tests for allocator reentry from error and deferred-free
        callbacks.
  - [x] Add tests for panic containment in `std` and test builds.
  - [x] Document which callback races remain caller obligations because Tier U
        constructors are unsafe.
  - Evidence: callback unit coverage now includes a deterministic registration
    model for stale snapshot hazards, ignored serialized process-global tests
    for nested LIFO restore of both callback guards, allocator reentry from
    error and deferred-free callbacks, deferred-free reentry suppression, and
    panic containment for both guard families. The safety-stress script runs
    those ignored Tier U tests with `--test-threads=1`, and the safe API,
    API inventory, ABI, and unsafe-audit docs now state the LIFO and
    concurrency obligations left to callers.
  - Verify: callback tests and any model tests pass under the safety-stress
    script.

- [x] Add artifact-level no-unwind tests.
  - [x] Build the release `c-abi` cdylib with panic abort.
  - [x] Trigger representative error, callback, and OOM paths from a C
        consumer.
  - [x] Assert Rust panic/unwind does not cross the C ABI boundary.
  - [x] Include callback panic containment where `std` or test builds catch
        panics.
  - Evidence: `scripts/check-no-unwind-artifact.py` builds the release `c-abi`
    cdylib in an isolated target directory, verifies the release profile uses
    `panic = "abort"`, rejects real unwind runtime/personality exports while
    requiring the aborting no-std personality shim, compiles and runs a C
    consumer over allocation, overflow/error callback, OOM/null, and
    deferred-free callback paths, and runs the serialized ignored callback tests
    that cover `std`/test panic containment.
  - Verify: no-unwind artifact test passes on Linux x86_64 GNU.

- [x] Add sustained thread-stress coverage for release candidates.
  - [x] Stress same-thread allocate/free.
  - [x] Stress cross-thread free.
  - [x] Stress abandoned page reclaim.
  - [x] Stress thread init/done.
  - [x] Stress process done and shutdown stats.
  - [x] Add fork/exec constraints only if they are claimed as supported.
  - Evidence: `scripts/check-release-thread-stress.py` builds a release
    `c-abi` cdylib in an isolated target directory and runs a C consumer that
    exercises sustained same-thread churn, cross-thread frees, abandoned-page
    visitation and reclaim, repeated `mi_thread_init`/`mi_thread_done`, final
    stats capture/printing, and `mi_process_done`. No fork/exec behavior is
    claimed by the supported target policy, so no fork/exec constraint was
    added.
  - Verify: stress script is part of release-candidate validation.

## P1 - Align CI and release gates

- [x] Make formatting a first-class validation workflow step.
  - [x] Add `scripts/check-fmt.sh` to `.github/workflows/validation.yml`.
  - [x] Consider changing `scripts/check-fmt.sh` to `cargo fmt --all --check`
        so it matches the project-required developer command.
  - [x] Keep `git diff --check` for whitespace errors.
  - Evidence: `.github/workflows/validation.yml` now runs
    `scripts/check-fmt.sh` before tests and lints. The script runs
    `cargo fmt --all --check` and keeps `git diff --check`.
  - Verify: validation workflow runs formatting before lints.

- [x] Add feature-sensitive ABI checks to CI.
  - [x] Run `scripts/check-feature-abi.sh` in validation or release CI.
  - [x] Include default, `secure`, `guarded`, `track-valgrind`, and
        `track-asan` symbol/layout checks as currently scripted.
  - [x] Add dynamic cdylib feature-mode symbol checks after dynamic manifests
        exist.
  - Evidence: `.github/workflows/validation.yml` has a separate
    `Feature-sensitive ABI` step that runs `scripts/check-feature-abi.sh`.
    The script reports each feature set by name, runs the existing
    symbol/layout checks for default, `secure`, `guarded`, `track-valgrind`,
    and `track-asan`, and then checks dynamic `c-abi` feature-mode manifests
    for those feature-sensitive cdylibs.
  - Verify: feature ABI job reports each feature set separately.

- [x] Add C and C++ artifact consumer checks to CI.
  - [x] Run the focused C static and dynamic consumer script.
  - [x] Run the focused `dlopen` script.
  - [x] Run the focused `LD_PRELOAD` script.
  - [x] Run the supported C++ override script.
  - [x] Keep broad behavioral parity available as a release-candidate job if it
        remains too slow for every pull request.
  - Evidence: `.github/workflows/validation.yml` builds release artifacts once,
    then runs `scripts/check-release-artifact-consumer.py`,
    `scripts/check-release-dlopen.py`, `scripts/check-release-ld-preload.py`,
    and `scripts/check-release-cxx-artifacts.py` with explicit step names for
    the `c-abi`, `interpose`, static-preview, dynamic, `dlopen`,
    `LD_PRELOAD`, and C++ override artifact modes. The broader
    `scripts/check-behavioral-parity.py` remains outside per-PR validation.
  - Verify: validation or release workflow names the artifact mode under test.

- [x] Align `docs/release.md` with existing hardening workflows.
  - [x] Add ASan to the release checklist.
  - [x] Add TSan to the release checklist.
  - [x] Add Valgrind tracking parity to the release checklist.
  - [x] Add Miri core unsafe validation to the release checklist.
  - [x] Add safety-stress to the release checklist.
  - [x] State which jobs must be green before a release tag.
  - Evidence: `docs/release.md` now requires the `linux-x86_64`,
    `linux-aarch64`, `macos`, `windows`, `safety-stress`, `miri`,
    `address-sanitizer`, `thread-sanitizer`, and `valgrind` workflow jobs to
    be green before creating a release tag. The release checklist also names
    the matching local hardening commands for safety-stress, Miri, ASan, TSan,
    and Valgrind tracking parity.
  - Verify: release checklist and `.github/workflows/validation.yml` agree.

- [x] Pin performance workflow toolchain consistently.
  - [x] Change `.github/workflows/performance.yml` from `stable` to the pinned
        release toolchain or document why benchmark gates intentionally use
        floating stable.
  - [x] Capture `scripts/env-info.sh` output in performance artifacts.
  - [x] Upload benchmark CSV and environment evidence.
  - Evidence: `.github/workflows/performance.yml` now uses the same pinned
    Rust `1.92.0` toolchain as validation, writes `scripts/env-info.sh` output
    to `target/criterion/benchmark-environment.txt`, and uploads that file
    together with `target/criterion/benchmark-thresholds-smoke.csv` in the
    `benchmark-evidence` artifact.
  - Verify: performance workflow artifacts include threshold and environment
    files.

- [x] Add package consumer matrix coverage.
  - [x] Keep the existing packaged safe-Rust consumer test.
  - [x] Add a default-feature packaged consumer if it differs from the current
        `std` consumer.
  - [x] Add an FFI artifact consumer test that does not rely on unpublished
        repo scripts.
  - [x] Add docs.rs-style documentation build verification if not already
        covered by `scripts/check-docs.sh`.
  - Evidence: `scripts/check-package.sh` still packages and runs the
    `std` safe-Rust consumer from the unpacked `.crate`, now also runs a
    default-feature packaged Rust consumer, runs a docs.rs-style
    `RUSTDOCFLAGS="--cfg docsrs" cargo doc --no-deps --features std` build
    against the unpacked package, and invokes
    `scripts/check-package-ffi-consumer.py`. The FFI consumer copies the staged
    release asset tree to a temporary downloaded-assets directory and compiles
    a C consumer using only the copied headers and `c-abi` cdylib, with no
    repository include paths or consumer-side release scripts.
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

- [x] Tier S/Tier U Rust API contract is frozen and diffed.
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
