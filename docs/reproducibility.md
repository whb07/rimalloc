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
`rust-toolchain.toml`. Lints are checked by:

```sh
scripts/check-lints.sh
```

## Benchmark Reproduction

Benchmark command lines and environment variables must be preserved in
`BENCHMARK_RESULTS.md`. Performance thresholds and noise handling are defined
in `docs/performance.md`.
