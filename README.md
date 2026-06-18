# mechanical_mimalloc

Mechanical Rust translation foundation for mimalloc 3.3.2.

This repository currently builds both:

- an `rlib` for Rust tests, benchmarks, and experimental integration, and
- a `cdylib` that exports mimalloc-compatible C ABI symbols.

The C ABI compatibility surface is the primary target today. The safe Rust API
is intentionally small and experimental while the allocator invariants are still
being audited.

## Quick Start

Use the Rust `GlobalAlloc` facade:

```rust
use mechanical_mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

Use the raw C-shaped facade only when you can uphold mimalloc's pointer,
layout, alignment, and threading rules:

```rust
unsafe {
    let p = mechanical_mimalloc::raw::mi_malloc(64);
    mechanical_mimalloc::raw::mi_free(p);
}
```

## Status

This crate is not published and `Cargo.toml` has `publish = false`. The crate
root exposes only `MiMalloc` and the explicit `raw` facade; implementation
modules are private so translation structure does not become an accidental
SemVer contract.

## Features

Default features are empty.

- `std`: opt into `std` for tooling, tests, examples, and benchmarks.
- `bench`: enable Criterion benchmark targets; implies `std`.
- `compare`: enable comparison-oriented benchmark and test code.
- `secure`: enable secure-mode padding and encoded free lists.
- `guarded`: enable guarded allocation sampling.
- `track-valgrind`: enable Valgrind tracking shims.
- `track-asan`: enable ASan tracking shims.

`track-valgrind` and `track-asan` are mutually exclusive.

## Supported Targets

The maintained target for the current port is Linux x86_64 GNU. Other upstream
mimalloc platforms remain part of the source translation, but they are not yet
claimed as maintained Rust targets until parity and CI evidence exists.
See `docs/platforms.md` and `docs/limitations.md` for the current support
matrix and caveats.

## Verification

Common local checks:

```sh
scripts/check-local.sh
```

Individual entrypoints are available under `scripts/` for formatting, lints,
feature sweeps, ABI symbols, layout, docs, packaging, benchmark smoke/full
runs, Miri, sanitizers, and environment capture.

Benchmark evidence is tracked in `BENCHMARK_RESULTS.md`; policy and thresholds
are documented in `docs/performance.md`. Performance-sensitive changes should
include fresh results for the changed benchmark set and a full report when hot
paths or allocator-wide behavior can move.

Additional policy documents live under `docs/`, including API inventory, ABI,
assembly and platform intrinsics, features, memory ordering, performance,
release, testing and hardening, unsafe audit, reproducibility, governance,
upstream synchronization, and dependency policy.

## Examples

Compiled examples live under `examples/` and cover the small Rust facade plus
the explicit raw C-shaped API:

```sh
cargo check --examples --features std
cargo run --example global_allocator --features std
```

## Safety Model

`raw` mirrors a C allocator API. Calling it is unsafe unless the caller respects
ownership, allocation origin, alignment, object lifetime, nullability, and
threading requirements. Rust safe wrappers are added only where those invariants
can be upheld without relying on caller discipline.
