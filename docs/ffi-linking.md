# FFI Linking

The production-supported C ABI artifact for the current release policy is the
explicit cdylib build. Default Cargo library builds produce an `rlib` for Rust
integration and do not produce a linkable C library.

The crates.io package is the Rust library distribution. Production FFI
libraries and C headers are distributed as GitHub release assets built from the
tagged repository checkout. Do not use the crates.io package as the source for
FFI artifact production: repo-only release scripts and vendored C headers are
intentionally not part of that package.

Build the dynamic C ABI/interposition artifact with:

```sh
cargo rustc --features interpose --lib --release -- --crate-type cdylib
```

For release asset staging, use:

```sh
scripts/build-release-artifacts.py
```

The script builds each artifact mode with an isolated `CARGO_TARGET_DIR`, copies
the outputs under `target/release-artifacts/artifacts/`, stages public headers
under `target/release-artifacts/include/`, generates link metadata under
`target/release-artifacts/lib/`, and writes
`target/release-artifacts/release-artifacts.json` with artifact paths, header
paths, link metadata paths, feature sets, target triple, profile, rustc
version, and SHA-256 checksums.

The staged headers are:

- `mimalloc.h` for the core C API.
- `mimalloc-stats.h` for stats APIs.
- `mimalloc-override.h` for C interposition users.
- `mimalloc-new-delete.h` for C++ operator new/delete users.

The staged link metadata is:

- `lib/pkgconfig/mechanical_mimalloc.pc`, whose default `Libs` line links the
  production `c-abi` cdylib and records include path, artifact path, `pthread`,
  `dl`, `m`, and artifact-mode notes. Preview static archive paths are exposed
  as variables, not as the default production link mode.
- `lib/cmake/mechanical_mimalloc/mechanical_mimalloc-config.cmake`, which
  defines `mechanical_mimalloc::c_abi`,
  `mechanical_mimalloc::interpose`,
  `mechanical_mimalloc::c_abi_static_preview`, and
  `mechanical_mimalloc::interpose_static_preview` imported targets.

Validate package metadata consumers from a temporary directory outside the
repository:

```sh
scripts/check-release-link-metadata.py
```

Static archives are preview-only. They are used by repository parity tests,
source-build experiments, and static archive audits, but they are not a
production-supported FFI distribution artifact until the static symbol
contract, collision tests, and checked static manifests are complete.

If a release candidate includes static archives for evaluation, the artifact
name must include a preview marker such as
`mechanical_mimalloc-static-preview`, and the release notes must state that the
archive is not a stable static ABI.
