# Release Policy

This repository is pre-release at version `0.1.0`, with packaging enabled.

## Production Contract

`docs/api-inventory.md` is the authoritative public API inventory for release
signoff unless a future release explicitly introduces a separate
`PUBLIC_API.md`. Release review must treat any mismatch between that inventory
and the source as release-blocking until the inventory, compatibility policy,
and source-shape checks agree.

The production contract is split into compatibility tiers:

- Tier S is the only stable safe Rust API surface.
- Tier U is public unsafe Rust API. It receives Rust SemVer review, but it is
  not Tier S and callers must uphold the documented unsafe obligations.
- Tier R is the unsafe raw Rust facade. It remains Rust-path unstable unless a
  later version explicitly stabilizes a subset.
- Tier C is a C ABI contract only. It does not imply Rust ABI, Rust layout, or
  Rust module-path stability.
- Tier I is private implementation machinery and is not public API.

Release signoff must confirm that no new crate-root public item was added
without inventory classification, compatibility review, and corresponding
source-shape check updates.

## Raw Facade Release Policy

The current release policy keeps the public `raw` module available as Tier R.
It is explicitly unsafe and Rust-path unstable. It exists for C-header parity,
repository tests, benchmarks, and low-level integrations that can uphold the
same pointer ownership, lifetime, layout, alignment, threading, and callback
contracts as the C API. Keeping `raw` available does not stabilize translated
implementation module paths or Rust layouts.

## Tier S/U API Diff Snapshot

Tier S and Tier U Rust source compatibility is tracked with
`docs/api-snapshots/tier-s-u.txt`. The checked snapshot excludes Tier R raw
paths and Tier C ABI symbols because those surfaces have separate raw-facade,
symbol, and layout checks.

After an intentional Tier S or Tier U API change, regenerate and review the
snapshot:

```sh
scripts/check-api-inventory.py --write-tier-su-snapshot
scripts/check-api-inventory.py
```

For release review against a previous tag:

```sh
git show <previous-tag>:docs/api-snapshots/tier-s-u.txt > /tmp/previous-tier-s-u.txt
diff -u /tmp/previous-tier-s-u.txt docs/api-snapshots/tier-s-u.txt
```

Any diff must be classified in `CHANGELOG.md` and reviewed against
`docs/compatibility.md`.

## Dynamic ABI Manifests

Tier C dynamic exports are tracked with checked manifests under
`docs/abi-manifests/` for the `c-abi` and `interpose` cdylib artifacts. Each
manifest records the artifact build command plus symbol name, binding,
visibility, version, and type.

After an intentional dynamic ABI change, regenerate and review the manifests:

```sh
scripts/check-public-symbols.py --write-dynamic-manifests
scripts/check-abi-symbols.sh
```

Any manifest diff must be classified in `CHANGELOG.md` and reviewed against
`docs/abi.md`.

For release review against a previous tag:

```sh
scripts/check-abi-diff.py --previous-ref <previous-tag>
```

For release review against an unpacked previous release asset directory:

```sh
scripts/check-abi-diff.py --previous-dir <previous-release>/abi-manifests
```

The diff check compares every checked manifest under `docs/abi-manifests/`.
That includes current cdylib manifests and any staticlib manifests added by the
static artifact policy. It classifies added, removed, possible-renamed,
visibility-changed, binding-changed, version-changed, and type-changed symbols.
Every intentional symbol change must be recorded in the changelog ABI section
with `ABI-CHANGE: <manifest> <symbol>`, or with
`ABI-CHANGE: <manifest> *` for an explicitly reviewed full-manifest baseline
or broad artifact-mode update.

## Static Archive Policy

The `staticlib` artifact is preview-only for the current release policy. It may
be built for repository parity checks, local source-build experiments, and
static archive audits, but it is not a production-supported FFI distribution
artifact until the static symbol contract, collision tests, and checked static
manifests are complete.

Release notes and release artifact names must make this visible. If static
archives are attached for evaluation before the production policy changes, use
a preview-marked name such as `mechanical_mimalloc-static-preview` and state
that the archive is not a stable static ABI.

## FFI Artifact Distribution

The chosen distribution model is a crates.io Rust crate plus GitHub release
assets for FFI consumers. The crates.io package is the Rust library
distribution and is validated by `scripts/check-package.sh`. Production C ABI
libraries, C headers, link metadata, checksums, ABI manifests, and
reproducibility notes are built from the tagged repository checkout and
attached to the GitHub release.

This model is intentional because repo-only release scripts and the vendored C
headers under `mimalloc-3.3.2/include/` are not included in the crate package.
FFI asset production therefore uses the full release checkout, not the
published crate tarball. A split Rust/FFI crate is deferred until there is a
separate supported FFI package contract.

Build release library artifacts with:

```sh
scripts/build-release-artifacts.py
```

The script builds the Rust `rlib`, `c-abi` cdylib, `c-abi` staticlib preview,
`interpose` cdylib, and `interpose` staticlib preview using separate
`CARGO_TARGET_DIR` values. Static preview artifacts are built with native-object
rustflags so C linker smoke tests do not depend on LLVM bitcode support. It
copies outputs under
`target/release-artifacts/artifacts/`, stages `mimalloc.h`,
`mimalloc-stats.h`, `mimalloc-override.h`, and `mimalloc-new-delete.h` under
`target/release-artifacts/include/`, generates
`lib/pkgconfig/mechanical_mimalloc.pc` and
`lib/cmake/mechanical_mimalloc/mechanical_mimalloc-config.cmake`, and writes
`target/release-artifacts/release-artifacts.json` with each artifact path,
header path, link metadata path, feature set, target triple, release profile,
rustc version, and SHA-256 checksum.

The dry run also writes release integrity files:

- `target/release-artifacts/SHA256SUMS` for every staged release file outside
  the temporary build directory.
- `target/release-artifacts/integrity/abi-symbols/*.tsv` for every staged
  cdylib/staticlib artifact.
- `target/release-artifacts/integrity/environment.txt`, captured with
  `scripts/env-info.sh`.
- `target/release-artifacts/integrity/benchmark-evidence.md`, which points to
  `docs/performance.md`, `BENCHMARK_RESULTS.md`, and the required benchmark
  commands.

Validate staged C consumers for dynamic and static `c-abi` and `interpose`
artifact modes without repository include paths:

```sh
scripts/check-release-artifact-consumer.py
```

Validate pkg-config and CMake consumers from a temporary directory outside the
repository:

```sh
scripts/check-release-link-metadata.py
```

Validate release integrity files from a fresh dry run:

```sh
scripts/check-release-integrity.py
```

Validate focused dynamic loading and interposition behavior:

```sh
scripts/check-release-dlopen.py
scripts/check-release-ld-preload.py
```

Validate C++ operator new/delete release artifact behavior:

```sh
scripts/check-release-cxx-artifacts.py
```

The validation workflow in `.github/workflows/validation.yml` is the required
pre-tag gate. Before creating a release tag, these jobs must be green for the
release commit:

- `linux-x86_64` (`Linux x86_64 full validation`)
- `linux-aarch64` (`Linux aarch64 smoke validation`)
- `macos` (`macOS smoke validation`)
- `windows` (`Windows smoke validation`)
- `safety-stress` (`Safety and stress tests`)
- `miri` (`Miri core unsafe validation`)
- `address-sanitizer` (`AddressSanitizer`)
- `thread-sanitizer` (`ThreadSanitizer`)
- `valgrind` (`Valgrind tracking parity`)

Before publishing or tagging a release:

1. Confirm public Rust API inventory.
2. Confirm no object ownership APIs such as `MiBox<T>`, `HeapBox<'heap, T>`,
   object constructors, or object destructors were added to the safe facade
   without a separate design covering construction failure, destructor and panic
   behavior, allocator/heap ownership, and threading traits.
3. Run `scripts/check-api-inventory.py`.
4. Run `scripts/check-ffi-boundary.py`.
5. Run `scripts/check-local.sh`.
6. Run `scripts/check-abi-symbols.sh` for exported C symbol inventory.
7. Run `scripts/check-abi-diff.py --previous-ref <previous-tag>` for Tier C
   release-to-release ABI review.
8. Run `scripts/check-static-link-collisions.py` for preview static archive
   link and collision evidence.
9. Run `scripts/check-release-artifact-consumer.py` and
   `scripts/check-release-link-metadata.py`.
10. Run `scripts/check-release-dlopen.py`.
11. Run `scripts/check-release-ld-preload.py`.
12. Run `scripts/check-release-cxx-artifacts.py`.
13. Run `scripts/check-release-integrity.py`.
14. Run `scripts/check-layout.sh` for ABI/layout audits.
15. Run audit drift checks: `scripts/check-unsafe-inventory.py --verify-builds`,
   `scripts/check-unsafe-audit.py`,
   `scripts/check-unsafe-audit-evidence.py`,
   `scripts/check-atomic-ordering-audit.py`,
   `scripts/check-pointer-provenance-audit.py`,
   `scripts/check-no-allocation-audit.py`, and
   `scripts/check-safe-boundary.py`.
16. Run `scripts/check-release-thread-stress.py`.
17. Run `scripts/check-safety-stress.sh`.
18. Run `bash scripts/check-miri-all.sh` with the workflow's nightly Miri
   toolchain.
19. Run `SANITIZER=address SANITIZER_TOOLCHAIN=nightly
   scripts/check-sanitizers.sh`.
20. Run `SANITIZER=thread SANITIZER_TOOLCHAIN=nightly
   scripts/check-sanitizers.sh`.
21. Run `scripts/check-valgrind-tracking-parity.py`.
22. Run `scripts/check-bench-full.sh` for hot-path or allocator-wide changes,
   using `docs/performance.md` for thresholds and noise handling.
23. Update `CHANGELOG.md`, `README.md`, and compatibility notes.
24. Capture `scripts/env-info.sh` output alongside benchmark evidence.

SemVer applies separately to safe Rust API, raw Rust facade, C ABI, raw layout,
feature behavior, MSRV, and supported target tiers.
`docs/compatibility.md` defines these domains and release classification rules.

## Staged Documentation Gates

The crate denies broken rustdoc links and warns on bare URLs today. A global
`missing_docs` warning or deny gate is intentionally staged until the default
`raw` facade no longer exposes broad mechanical-port internals. Before enabling
that lint globally:

1. Keep raw internals private according to `TODO.md`.
2. Confirm every Tier S safe Rust item has examples and error/panic notes.
3. Confirm every public unsafe Tier R item has a `# Safety` section or is kept
   out of the public raw facade.
4. Run `scripts/check-docs.sh`.

Related policy documents:

- `docs/api-inventory.md`
- `docs/safe-api.md`
- `docs/abi.md`
- `docs/ffi-linking.md`
- `docs/raw-facade-migration.md`
- `docs/assembly-intrinsics.md`
- `docs/compatibility.md`
- `docs/features.md`
- `docs/memory-ordering.md`
- `docs/platforms.md`
- `docs/performance.md`
- `docs/reproducibility.md`
- `docs/testing.md`
- `docs/strict-types.md`
- `docs/governance.md`
- `docs/limitations.md`
- `docs/upstream.md`
- `../UPSTREAM_DIFF.md`
- `docs/dependencies.md`
