# Reproducibility

This repository pins the local Rust toolchain through `rust-toolchain.toml`.

## Toolchain

- Rust channel: `1.92.0`
- Required components: `rustfmt`, `clippy`
- Edition: Rust 2024
- CI installs the same pinned toolchain for maintained-target validation. Other
  platform jobs are smoke checks unless `docs/platforms.md` promotes them.

## Capturing Environment Evidence

Use:

```sh
scripts/env-info.sh
```

Include that output with full benchmark reports, ABI investigations, platform
reports, and release sign-off notes.

## Release Artifact Reproduction

Release artifacts are produced from a clean repository checkout, not from the
published crate package. Use the signed release tag or exact release commit, and
start from a worktree where:

```sh
git status --short
git submodule status --recursive
```

prints no local changes or unexpected submodule drift. Do not rely on local
Cargo aliases or ignored build outputs. If the release uses native tool
overrides, set `CC`, `CXX`, and `AR` explicitly and record them in the release
notes.

The exact Rust toolchain is the pinned `rust-toolchain.toml` toolchain:

- Rust channel: `1.92.0`
- Required components: `rustfmt`, `clippy`

Confirm the active toolchain and installed components before building:

```sh
rustup show active-toolchain
rustup component list --installed
rustc --version --verbose
cargo --version --verbose
```

Build and validate the release payload with:

```sh
cargo fetch --locked
scripts/build-release-artifacts.py
scripts/check-release-integrity.py
```

The expected checksum source is
`target/release-artifacts/SHA256SUMS`. The same SHA-256 values are recorded in
`target/release-artifacts/release-artifacts.json` for every staged artifact,
header, link metadata file, and integrity file. Release notes must attach or
link `SHA256SUMS`, `SHA256SUMS.sig`, and the release public key fingerprint so
consumers can verify the downloaded payload.

ABI symbol manifests are written under
`target/release-artifacts/integrity/abi-symbols/`. A reproduction checkout must
regenerate the same artifact names and the same ABI symbol manifest entries for
the binary artifact modes:

- `c-abi-cdylib`
- `c-abi-static-preview`
- `interpose-cdylib`
- `interpose-static-preview`

Checksum differences between clean checkouts are release-blocking unless the
release owner documents a concrete environment or toolchain difference and
confirms that the artifact inventory and ABI symbol manifests still match.

Capture environment evidence after the artifact build:

```sh
scripts/env-info.sh > target/release-artifacts/integrity/environment.txt
```

The artifact builder captures the same file automatically during release
staging. Keep that file with the release payload and benchmark evidence.

For the repository-level reproduction gate, run:

```sh
scripts/check-release-reproducibility.py
```

The script materializes two fresh tracked-file checkouts, builds release
artifacts in both, and compares the artifact inventory plus normalized ABI
symbol manifests. It intentionally excludes untracked local files and detached
signature bytes from the comparison.

## Formatting

Formatting uses `rustfmt.toml` and is checked by:

```sh
scripts/check-fmt.sh
```

## Linting

Clippy uses `clippy.toml` with the crate MSRV from `Cargo.toml` and
`rust-toolchain.toml`. The lint deny policy lives in `Cargo.toml`, including
Rust warnings and the `clippy::all`, `clippy::pedantic`, `clippy::nursery`, and
`clippy::cargo` groups.

For direct Clippy runs, use:

```sh
cargo clippy --workspace --all-targets --all-features
```

Repository checkouts may define shorter local aliases, but package-facing
commands use the full Cargo invocation because aliases are not part of the
published crate.

The full repository lint entrypoint is:

```sh
scripts/check-lints.sh
```

Use the script when you need the repository's feature matrix checks, including
the separate tracking feature builds and their aggregate tooling combination.

Package-level Clippy allowances are intentional mechanical-translation debt, not
the style target for new Rust facade code. New allowances should be local and
justified by C ABI parity, translated C shape, or measured hot-path code shape;
Clippy, code review, and the normal diff are the enforcement mechanism.

## Benchmark Reproduction

Benchmark command lines and environment variables must be preserved in
`BENCHMARK_RESULTS.md`. Performance thresholds and noise handling are defined
in `docs/performance.md`.
