# Reproducibility

This repository pins the local Rust toolchain through `rust-toolchain.toml`.

## Toolchain

- Rust channel: `1.92.0`
- Required components: `rustfmt`, `clippy`
- Edition: Rust 2024

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
cargo clippy-strict
```

This Cargo alias expands to `cargo clippy --all-targets --all-features`.
Running plain `cargo clippy` applies the same lint deny policy, but only to
Cargo's default target and feature selection.

The full repository lint entrypoint is:

```sh
scripts/check-lints.sh
```

Use the script when you need the repository's feature matrix checks, including
mutually exclusive feature combinations that cannot be represented by one
`--all-features` invocation.

## Benchmark Reproduction

Benchmark command lines and environment variables must be preserved in
`BENCHMARK_RESULTS.md`. Performance thresholds and noise handling are defined
in `docs/performance.md`.
