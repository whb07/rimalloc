# Feature Policy

Default features are intentionally empty. Optimized Linux non-test builds use
`no_std` unless the `std` feature is enabled.

## Features

- `std`: tooling/test/docs support. This is opt-in.
- `bench`: Criterion benchmarks; implies `std`.
- `compare`: comparison benchmark/test code.
- `secure`: secure-mode padding and encoded free lists.
- `guarded`: guarded allocation sampling.
- `track-valgrind`: Valgrind tracking shims.
- `track-asan`: ASan tracking shims.

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
cargo check --benches --features bench
cargo check --benches --features "bench compare"
```
