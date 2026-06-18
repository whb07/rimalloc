# Release Policy

This repository is pre-release and `publish = false`.

Before publishing or tagging a release:

1. Confirm public Rust API inventory.
2. Run `scripts/check-api-inventory.py`.
3. Run `scripts/check-ffi-boundary.py`.
4. Run `scripts/check-local.sh`.
5. Run `scripts/check-abi-symbols.sh` for exported C symbol inventory.
6. Run `scripts/check-layout.sh` for ABI/layout audits.
7. Run `scripts/check-bench-full.sh` for hot-path or allocator-wide changes,
   using `docs/performance.md` for thresholds and noise handling.
8. Update `CHANGELOG.md`, `README.md`, and compatibility notes.
9. Capture `scripts/env-info.sh` output alongside benchmark evidence.

SemVer applies separately to safe Rust API, raw Rust facade, C ABI, raw layout,
feature behavior, MSRV, and supported target tiers.
`docs/compatibility.md` defines these domains and release classification rules.

Related policy documents:

- `docs/api-inventory.md`
- `docs/abi.md`
- `docs/assembly-intrinsics.md`
- `docs/compatibility.md`
- `docs/features.md`
- `docs/memory-ordering.md`
- `docs/platforms.md`
- `docs/performance.md`
- `docs/reproducibility.md`
- `docs/testing.md`
- `docs/strict-types.md`
- `docs/governance.md`
- `docs/limitations.md`
- `docs/upstream.md`
- `../UPSTREAM_DIFF.md`
- `docs/dependencies.md`
