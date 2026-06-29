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
the style target for new Rust facade code. `tests/release_fast_path_audit.rs`
keeps both the package allowance set and localized source allowances on fixed
inventories; new allowances should be local and justified by C ABI parity,
translated C shape, or measured hot-path code shape.

## Benchmark Reproduction

Benchmark command lines and environment variables must be preserved in
`BENCHMARK_RESULTS.md`. Performance thresholds and noise handling are defined
in `docs/performance.md`.
