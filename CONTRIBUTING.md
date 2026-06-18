# Contributing

Keep changes surgical and verified. This allocator is a mechanical translation,
so preserving upstream behavior is usually more important than idiomatic
rewrites.

## Expectations

- State the upstream behavior or invariant being changed.
- Keep public Rust API changes behind the crate facade, not new crate-root
  implementation modules.
- Add or update tests for allocator behavior, ABI/layout behavior, and hot-path
  assumptions when relevant.
- Document new unsafe assumptions in `docs/unsafe-audit.md`.
- Include benchmark evidence for allocator hot-path or broad workload changes.

## Local Checks

Run the checks that match the change:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo check --features std
cargo check --features secure
cargo check --features guarded
cargo check --features track-valgrind
cargo check --features track-asan
cargo check --benches --features bench
```

