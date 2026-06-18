# Release Policy

This repository is pre-release and `publish = false`.

Before publishing or tagging a release:

1. Confirm public Rust API inventory.
2. Confirm exported C symbol inventory.
3. Run feature sweeps with `track-valgrind` and `track-asan` separately.
4. Run ABI/layout audits.
5. Run benchmark smoke tests and a full benchmark report for hot-path changes.
6. Update `CHANGELOG.md`, `README.md`, and compatibility notes.
7. Run `cargo package --list` and `cargo package`.

SemVer applies separately to safe Rust API, raw Rust facade, C ABI, raw layout,
feature behavior, MSRV, and supported target tiers.

