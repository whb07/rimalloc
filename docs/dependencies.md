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
Criterion, libc, and Rand.

## Review Rule

Any dependency change must include a changelog entry and should explain why a
small local implementation or existing standard/library API is insufficient.
