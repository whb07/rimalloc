# Feature Policy

Default features are intentionally empty. Linux non-test builds use `no_std`
unless the `std` feature is enabled.

Rust applications that link `std` must enable the `std` feature for this crate.
Without it, the Linux build supplies its own panic handler, conflicting with
`std`'s handler. This requirement applies to the global allocator, safe wrappers,
and raw facade alike; `std` is not just a tooling feature. It does not enable
C interposition or select the application's global allocator.

The default Linux `no_std` build includes panic-handler ownership for standalone
allocator artifacts. It is not a panic-handler-free library for embedding in an
arbitrary `no_std` application that supplies another handler.

## Features

- `std`: integration with Rust applications that link `std`, plus tooling,
  tests, docs, and examples support. This is opt-in.
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

When `track-valgrind` and `track-asan` are both enabled by aggregate tooling
builds, Valgrind mode takes precedence.

## Feature Sweep

Run tracking features separately and in the aggregate all-features combination:

```sh
cargo check
cargo check --lib --no-default-features
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
cargo check --examples --features std
cargo doc --no-deps --no-default-features
cargo rustc --features interpose --lib --release -- --crate-type cdylib
cargo check --benches --features bench
cargo check --benches --features "bench compare"
cargo hack check --feature-powerset --exclude-features bench,compare,default,interpose --no-dev-deps
```

The powerset check covers the library/runtime feature combinations. `bench` and
`compare` stay in explicit benchmark-target checks because they exercise
benchmark-only code paths, `interpose` is checked with the explicit cdylib build
command, and `default` is intentionally empty.

Repository examples are `std`-only executable targets and each Rust example in
`Cargo.toml` uses `required-features = ["std"]`. The default no-std contract is
checked through the library build and no-default rustdoc generation, while
doctests and examples are checked explicitly under `std`.
