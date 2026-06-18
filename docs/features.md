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

`track-valgrind` and `track-asan` are mutually exclusive and are guarded by a
compile-time error in `src/track.rs`.

## Feature Sweep

Run mutually exclusive tracking features separately:

```sh
cargo check
cargo check --features std
cargo check --features secure
cargo check --features guarded
cargo check --features track-valgrind
cargo check --features track-asan
cargo check --benches --features bench
```

