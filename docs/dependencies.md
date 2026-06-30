# Dependency Policy

The allocator runtime should remain dependency-free unless a future dependency
is justified by allocator correctness, platform coverage, or a clearly bounded
tooling need.

## Runtime Dependencies

Normal dependencies are intentionally empty. Adding one requires documenting:

- allocation behavior,
- `std` and `no_std` impact,
- MSRV impact,
- license compatibility,
- unsafe code and audit scope,
- platform support,
- panic and unwinding behavior.

## Development Dependencies

Development dependencies are allowed for tests, benchmarks, and tooling when
they do not affect release allocator behavior. Current dev dependencies are
Criterion, libc, Loom, and Rand.

## Release Dependency Report

Release dry runs generate `target/release-artifacts/integrity/dependencies.md`
from:

```sh
cargo metadata --locked --format-version 1 --all-features
```

The report lists direct runtime, build, and development dependencies, the full
resolved third-party package graph for the all-features release validation
scope, and native build-tool requirements. Runtime dependencies come only from
`[dependencies]`, which is intentionally empty for the current release policy.
Development dependencies and their transitive packages are test, benchmark, and
tooling inputs; they are not allocator runtime dependencies.

The report also records required native tools used by release builds and
checks, including the C compiler (`CC` or `cc`), C++ compiler (`CXX` or `c++`),
archiver (`AR` or `ar`), symbol readers (`readelf` and `nm`), Rust toolchain
commands, and the SSH signing tool.

## Review Rule

Any dependency change must include a changelog entry and should explain why a
small local implementation or existing standard/library API is insufficient.
