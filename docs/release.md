# Release Policy

This repository is pre-release at version `0.1.0`, with packaging enabled.

## Production Contract

`docs/api-inventory.md` is the authoritative public API inventory for release
signoff unless a future release explicitly introduces a separate
`PUBLIC_API.md`. Release review must treat any mismatch between that inventory
and the source as release-blocking until the inventory, compatibility policy,
and source-shape checks agree.

The production contract is split into compatibility tiers:

- Tier S is the only stable safe Rust API surface.
- Tier U is public unsafe Rust API. It receives Rust SemVer review, but it is
  not Tier S and callers must uphold the documented unsafe obligations.
- Tier R is the unsafe raw Rust facade. It remains Rust-path unstable unless a
  later version explicitly stabilizes a subset.
- Tier C is a C ABI contract only. It does not imply Rust ABI, Rust layout, or
  Rust module-path stability.
- Tier I is private implementation machinery and is not public API.

Release signoff must confirm that no new crate-root public item was added
without inventory classification, compatibility review, and corresponding
source-shape check updates.

## Raw Facade Release Policy

The current release policy keeps the public `raw` module available as Tier R.
It is explicitly unsafe and Rust-path unstable. It exists for C-header parity,
repository tests, benchmarks, and low-level integrations that can uphold the
same pointer ownership, lifetime, layout, alignment, threading, and callback
contracts as the C API. Keeping `raw` available does not stabilize translated
implementation module paths or Rust layouts.

## Tier S/U API Diff Snapshot

Tier S and Tier U Rust source compatibility is tracked with
`docs/api-snapshots/tier-s-u.txt`. The checked snapshot excludes Tier R raw
paths and Tier C ABI symbols because those surfaces have separate raw-facade,
symbol, and layout checks.

After an intentional Tier S or Tier U API change, regenerate and review the
snapshot:

```sh
scripts/check-api-inventory.py --write-tier-su-snapshot
scripts/check-api-inventory.py
```

For release review against a previous tag:

```sh
git show <previous-tag>:docs/api-snapshots/tier-s-u.txt > /tmp/previous-tier-s-u.txt
diff -u /tmp/previous-tier-s-u.txt docs/api-snapshots/tier-s-u.txt
```

Any diff must be classified in `CHANGELOG.md` and reviewed against
`docs/compatibility.md`.

## Dynamic ABI Manifests

Tier C dynamic exports are tracked with checked manifests under
`docs/abi-manifests/` for the `c-abi` and `interpose` cdylib artifacts. Each
manifest records the artifact build command plus symbol name, binding,
visibility, version, and type.

After an intentional dynamic ABI change, regenerate and review the manifests:

```sh
scripts/check-public-symbols.py --write-dynamic-manifests
scripts/check-abi-symbols.sh
```

Any manifest diff must be classified in `CHANGELOG.md` and reviewed against
`docs/abi.md`.

For release review against a previous tag:

```sh
scripts/check-abi-diff.py --previous-ref <previous-tag>
```

For release review against an unpacked previous release asset directory:

```sh
scripts/check-abi-diff.py --previous-dir <previous-release>/abi-manifests
```

The diff check compares every checked manifest under `docs/abi-manifests/`.
That includes current cdylib manifests and any staticlib manifests added by the
static artifact policy. It classifies added, removed, possible-renamed,
visibility-changed, binding-changed, version-changed, and type-changed symbols.
Every intentional symbol change must be recorded in the changelog ABI section
with `ABI-CHANGE: <manifest> <symbol>`, or with
`ABI-CHANGE: <manifest> *` for an explicitly reviewed full-manifest baseline
or broad artifact-mode update.

Before publishing or tagging a release:

1. Confirm public Rust API inventory.
2. Run `scripts/check-api-inventory.py`.
3. Run `scripts/check-ffi-boundary.py`.
4. Run `scripts/check-local.sh`.
5. Run `scripts/check-abi-symbols.sh` for exported C symbol inventory.
6. Run `scripts/check-abi-diff.py --previous-ref <previous-tag>` for Tier C
   release-to-release ABI review.
7. Run `scripts/check-layout.sh` for ABI/layout audits.
8. Run audit drift checks: `scripts/check-unsafe-audit.py`,
   `scripts/check-atomic-ordering-audit.py`,
   `scripts/check-pointer-provenance-audit.py`,
   `scripts/check-no-allocation-audit.py`, and
   `scripts/check-safe-boundary.py`.
9. Run `scripts/check-bench-full.sh` for hot-path or allocator-wide changes,
   using `docs/performance.md` for thresholds and noise handling.
10. Update `CHANGELOG.md`, `README.md`, and compatibility notes.
11. Capture `scripts/env-info.sh` output alongside benchmark evidence.

SemVer applies separately to safe Rust API, raw Rust facade, C ABI, raw layout,
feature behavior, MSRV, and supported target tiers.
`docs/compatibility.md` defines these domains and release classification rules.

## Staged Documentation Gates

The crate denies broken rustdoc links and warns on bare URLs today. A global
`missing_docs` warning or deny gate is intentionally staged until the default
`raw` facade no longer exposes broad mechanical-port internals. Before enabling
that lint globally:

1. Narrow or gate raw internals according to `TODO.md`.
2. Confirm every Tier S safe Rust item has examples and error/panic notes.
3. Confirm every public unsafe Tier R item has a `# Safety` section or is moved
   behind an unstable-internals gate.
4. Run `scripts/check-docs.sh`.

Related policy documents:

- `docs/api-inventory.md`
- `docs/safe-api.md`
- `docs/abi.md`
- `docs/raw-facade-migration.md`
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
