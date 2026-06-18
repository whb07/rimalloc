# Release Policy

This repository is pre-release and `publish = false`.

Before publishing or tagging a release:

1. Confirm public Rust API inventory.
2. Run `scripts/check-local.sh`.
3. Run `scripts/check-abi-symbols.sh` for exported C symbol inventory.
4. Run `scripts/check-layout.sh` for ABI/layout audits.
5. Run `scripts/check-bench-full.sh` for hot-path or allocator-wide changes,
   using `docs/performance.md` for thresholds and noise handling.
6. Update `CHANGELOG.md`, `README.md`, and compatibility notes.
7. Capture `scripts/env-info.sh` output alongside benchmark evidence.

SemVer applies separately to safe Rust API, raw Rust facade, C ABI, raw layout,
feature behavior, MSRV, and supported target tiers.

Related policy documents:

- `docs/api-inventory.md`
- `docs/abi.md`
- `docs/features.md`
- `docs/platforms.md`
- `docs/performance.md`
- `docs/limitations.md`
- `docs/upstream.md`
- `docs/dependencies.md`
