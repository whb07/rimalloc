# Feature Policy

Default features are intentionally empty. Linux non-test builds use `no_std`
unless the `std` feature is enabled.

## Features

- `std`: tooling/test/docs support. This is opt-in.
- `bench`: Criterion benchmarks; implies `std`.
- `compare`: comparison benchmark/test code.
- `bench-diagnostics`: benchmark-only hotspot counters for local profiling.
- `secure`: secure-mode padding and encoded free lists.
- `guarded`: guarded allocation sampling.
- `track-valgrind`: Valgrind tracking shims.
- `track-asan`: ASan tracking shims.
- `c-header-macros`: Rust macro translations of mimalloc C header
  convenience macros. This is Tier R raw compatibility surface and is off by
  default.
- `c-abi`: build-script setup for C ABI artifacts.
- `cxx-overrides`: C++ operator new/delete override shim; implies `c-abi`.
- `tls-shim`: Linux initial-exec TLS shim; implies `c-abi`.
- `interpose`: aggregate C ABI/interposition artifact mode; implies `c-abi`,
  `cxx-overrides`, and `tls-shim`.
- `unstable-raw-internals`: repository-audit access to translated
  implementation modules. This is not public API.

When `track-valgrind` and `track-asan` are both enabled by aggregate tooling
builds, Valgrind mode takes precedence.

## Feature Sweep

Run tracking features separately and in the aggregate all-features combination:

```sh
cargo check
cargo check --features std
cargo check --features secure
cargo check --features guarded
cargo check --features compare
cargo check --features track-valgrind
cargo check --features track-asan
cargo check --features "track-valgrind track-asan"
cargo check --features "secure guarded std"
cargo check --features "std c-header-macros"
cargo check --features "std c-abi"
cargo rustc --features interpose --lib --release -- --crate-type cdylib
cargo check --benches --features bench
cargo check --benches --features "bench compare"
cargo hack check --feature-powerset --exclude-features bench,compare,default,interpose --no-dev-deps
```

The powerset check covers the library/runtime feature combinations. `bench` and
`compare` stay in explicit benchmark-target checks because they exercise
benchmark-only code paths, `interpose` is checked with the explicit cdylib build
command, and `default` is intentionally empty.
