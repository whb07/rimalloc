# Contributing

Keep changes surgical and verified. This allocator is a mechanical translation,
so preserving upstream behavior is usually more important than idiomatic
rewrites.

## Expectations

- State the upstream behavior or invariant being changed.
- Follow the crate-specific Rust style guide in `docs/rust-style.md` for
  Rust-facing naming, constructors, errors, accessors, `Copy`/`Clone`, unsafe
  comments, and hot-path wrapper changes.
- Keep public Rust API changes behind the crate facade, not new crate-root
  implementation modules.
- Add or update tests for allocator behavior, ABI/layout behavior, and hot-path
  assumptions when relevant.
- Document new unsafe assumptions in `docs/unsafe-audit.md`.
- Include benchmark evidence for allocator hot-path or broad workload changes.
- Use the unsafe design-change and benchmark-report templates when changing
  allocator invariants or performance-sensitive paths.

## Local Checks

Run the checks that match the change:

```sh
scripts/check-local.sh
```

Use the narrower `scripts/check-*.sh` entrypoints when a change only needs part
of the local verification matrix.
