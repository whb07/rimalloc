# mechanical_mimalloc

Mechanical Rust translation foundation for mimalloc 3.3.2.

Use this crate in one of three ways:

1. as a Rust global allocator,
2. through safe Rust-owned byte, heap, stats, and options wrappers,
3. as an explicit mimalloc-compatible C ABI/interposition artifact.

The crate also exposes a raw C-shaped Rust facade for low-level integration, but
that surface is unsafe and unstable as a Rust path API.

The default library build produces:

- an `rlib` for Rust integration, tests, benchmarks, and examples.

The dynamic C ABI/interposition artifact is opt-in:

```sh
cargo rustc --features interpose --lib --release -- --crate-type cdylib
```

## Choose a Path

### 1. Rust Global Allocator

Use `MiMalloc` when you want Rust standard-library allocations in the process to
go through this allocator:

```rust
use mechanical_mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

### 2. Safe Rust Owned Bytes, Heaps, Stats, and Options

Use the safe facade when you want Rust-native ownership and typed errors instead
of C null/error conventions:

```rust
use core::alloc::Layout;
use mechanical_mimalloc::{Allocation, GlobalOptions, Heap, OptionKey, StatsSnapshot};

let layout = Layout::from_size_align(128, 16).unwrap();
let allocation = Allocation::zeroed(layout).unwrap();
assert_eq!(allocation.len(), 128);

let heap = Heap::new().unwrap();
let heap_allocation = heap.allocate_zeroed(Layout::from_size_align(64, 16).unwrap()).unwrap();
assert_eq!(heap_allocation.len(), 64);

let _max_warnings = GlobalOptions::get(OptionKey::MaxWarnings);
let _stats = StatsSnapshot::capture().unwrap();
```

The safe API intentionally owns bytes, not arbitrary Rust objects. Object
ownership APIs such as `MiBox<T>` are deferred until construction failure,
destructor behavior, panic behavior, heap ownership, and threading traits have a
separate design.

### 3. C ABI Artifact and Interposition

Use the opt-in C ABI artifact when you need mimalloc-compatible dynamic symbols
or `LD_PRELOAD`/malloc-family interposition:

```sh
cargo rustc --features interpose --lib --release -- --crate-type cdylib
```

Production C ABI libraries, C headers, link metadata, checksums, and ABI
manifests are shipped as GitHub release assets built from the tagged repository
checkout. The crates.io package is the Rust library distribution.

See `docs/ffi-linking.md`, `docs/c-mimalloc-migration.md`, `docs/abi.md`,
`docs/symbol-versioning.md`, and `docs/release.md` for artifact modes, headers,
link metadata, migration from C mimalloc, symbol policy, and release
validation.

### Unsafe Raw Facade

Use `mechanical_mimalloc::raw` only when you can uphold mimalloc's pointer,
layout, alignment, allocation-origin, lifetime, nullability, and threading
rules. The raw Rust path surface is not stabilized as a Rust SemVer API.

```rust
unsafe {
    let p = mechanical_mimalloc::raw::mi_malloc(64);
    mechanical_mimalloc::raw::mi_free(p);
}
```

## Distribution Model

The crates.io package is the Rust library distribution. Production C ABI
libraries, C headers, link metadata, checksums, and ABI manifests are shipped as
GitHub release assets built from the tagged repository checkout, not as files
generated from the crates.io package. Repo-only release scripts and vendored C
headers are intentionally outside the crate package, so FFI asset production
uses the full release checkout and the commands documented in
`docs/ffi-linking.md` and `docs/release.md`.

The C ABI compatibility surface and the crate-root safe Rust facade are both
tracked by source-shape, layout, symbol, lint, package, and benchmark gates.

## API Surfaces

- Safe Rust facade: crate-root `MiMalloc`, owned allocation wrappers, heap
  wrappers, supported options, non-printing stats snapshots, process
  information, and their typed error/status types.
- Unsafe-adjacent Rust callbacks: `mechanical_mimalloc::unsafe_callbacks`,
  borrowed RAII callback guards with process-global synchronization and lifetime
  obligations.
- Raw Rust facade: `mechanical_mimalloc::raw`, a C-shaped Rust path for
  low-level tests, benchmarks, and integration code that can uphold mimalloc's
  unsafe pointer and lifetime contracts. The raw Rust path surface is not
  stabilized as a Rust SemVer API yet.
- Dynamic C ABI: the opt-in `cdylib` export surface, which preserves
  mimalloc-compatible C symbols, layouts, callbacks, and calling conventions
  when built with the explicit interposition feature set. This is tracked
  separately from Rust SemVer.
- Static C archive: preview-only for repository parity and source-build
  experiments until the static archive symbol contract, collision behavior,
  and release manifests are frozen. It is not a production-supported FFI
  artifact in the current release policy.

See `docs/safe-api.md`, `docs/abi.md`, `docs/ffi-linking.md`, and
`docs/api-inventory.md` for the current boundary policy and inventory.

## Status

This crate is pre-release at version `0.1.0` and is configured for packaging.
The crate root exposes only the intended safe Rust facade, the explicit
`unsafe_callbacks` namespace, and the `raw` facade; implementation modules are
private so translation structure does not become an accidental SemVer contract.
The raw Rust facade is unstable as a Rust path surface; use it only behind an
explicit unsafe boundary and expect inventory or migration notes before treating
paths as stable.
Maintained platform support is currently Linux x86_64 GNU. Other upstream
mimalloc platform code may be present in the translation but is not claimed as a
maintained Rust target without parity and CI evidence.

## Features

Default features are empty.

- `std`: opt into `std` for tooling, tests, examples, and benchmarks. Linux
  builds without this feature use `no_std`.
- `bench`: enable Criterion benchmark targets; implies `std`.
- `compare`: enable comparison-oriented benchmark and test code.
- `bench-diagnostics`: enable benchmark-only hotspot counters.
- `secure`: enable secure-mode padding and encoded free lists.
- `guarded`: enable guarded allocation sampling.
- `track-valgrind`: enable Valgrind tracking shims.
- `track-asan`: enable ASan tracking shims.
- `c-header-macros`: expose raw mimalloc C-header compatibility macros.
- `c-abi`: enable build-script setup for C ABI artifacts.
- `cxx-overrides`: compile and link C++ operator new/delete overrides; implies
  `c-abi`.
- `tls-shim`: compile and link the Linux initial-exec TLS shim; implies
  `c-abi`.
- `interpose`: aggregate C ABI/interposition build mode; implies `c-abi`,
  `cxx-overrides`, and `tls-shim`.

When `track-valgrind` and `track-asan` are both enabled by aggregate tooling
builds, Valgrind mode takes precedence.

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

For direct Clippy runs, use the full workspace command:

```sh
cargo clippy --workspace --all-targets --all-features
```

Lint levels are defined in `Cargo.toml`, so plain `cargo clippy` uses the same
deny policy for Cargo's default target/feature selection. The
`scripts/check-lints.sh` entrypoint remains the compatibility sweep for the
tracked feature matrix.

Benchmark evidence is tracked in `BENCHMARK_RESULTS.md`; policy and thresholds
are documented in `docs/performance.md`. Performance-sensitive changes should
include fresh results for the changed benchmark set and a full report when hot
paths or allocator-wide behavior can move.

Additional policy documents live under `docs/`, including API inventory, ABI,
safe API policy, assembly and platform intrinsics, compatibility, features,
memory ordering, performance, release, Rust safety model, testing and
hardening, strict types, unsafe audit, reproducibility, governance, upstream
synchronization, and dependency policy.
Concrete local differences from upstream mimalloc are tracked in
`UPSTREAM_DIFF.md`.

## Examples

Compiled examples live under `examples/` and cover the small Rust facade plus
the explicit raw C-shaped API:

```sh
cargo check --examples --features std
cargo run --example global_allocator --features std
cargo run --example allocation --features std
```

## Safety Model

See `docs/rust-safety-model.md` for the Rust-facing safety model. In short,
safe wrappers are added only where ownership, initialization, heap lifetime,
nullability, threading, callback, and raw pointer invariants can be upheld
without caller discipline. `raw` mirrors a C allocator API and remains unsafe
and unstable as a Rust path surface.
