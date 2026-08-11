# Upstream Synchronization Policy

This repository tracks mimalloc 3.3.2 as the upstream behavioral and ABI
baseline. Deliberate local differences and sync-time recheck items are recorded
in `../UPSTREAM_DIFF.md`.

## Review Cadence

Review the vendored mimalloc baseline at least quarterly and before every
release candidate. Review immediately when upstream publishes a security
release, CVE, allocator-corruption fix, ABI/header change, platform primitive
change, guarded/secure-mode change, or performance-sensitive allocator change.

Each review records the latest upstream tag or commit inspected, whether the
vendored `mimalloc-3.3.2/` tree remains current, and whether a sync issue or
branch is required. Do not update vendored sources without updating parity,
ABI, release, and benchmark evidence in the same change series.

## Required Upstream Evidence

Before claiming synchronization with a newer upstream:

1. Record the upstream tag or commit.
2. Update public header and ABI/layout comparisons.
3. Review exported symbols, C++ operator overrides, callbacks, and option/stat
   definitions.
4. Re-run behavioral parity tests and platform-specific audits.
5. Re-run benchmark smoke tests and full benchmark reports for hot-path changes.
6. Record local deviations and unresolved mismatches in `PARITY.md` and
   `../UPSTREAM_DIFF.md`.

When upstream public headers or C ABI declarations change, re-run:

```sh
scripts/check-public-abi.py
scripts/check-public-symbols.py
scripts/check-public-symbols.py --missing-only
scripts/check-public-symbols.py --write-dynamic-manifests
scripts/check-abi-symbols.sh
scripts/check-c-link-public-api.py
scripts/check-c-baseline-missing-exports.py
scripts/check-release-artifact-consumer.py
scripts/check-release-link-metadata.py
```

When upstream implementation behavior changes, re-run:

```sh
scripts/check-behavioral-parity.py
scripts/check-cxx-bad-alloc-divergence.py
scripts/check-cxx-operator-new-parity.py
scripts/check-secure-mode-parity.py
scripts/check-guarded-mode-parity.py
scripts/check-asan-tracking-parity.py
scripts/check-valgrind-tracking-parity.py
scripts/check-unsafe-inventory.py --verify-builds
scripts/check-strict-types.py
scripts/check-unsafe-proofs.py --report
scripts/check-concurrency-models.sh
scripts/check-safety-stress.sh
scripts/check-fuzz-smoke.sh
```

For allocator hot paths, page/arena behavior, platform primitives, guarded or
secure paths, C++ overrides, or release artifact modes, also run the release
validation commands in `docs/release.md` that cover the affected surface.

## Local Deviations

Local Rust-only helper exports must be classified in
`symbols/public-symbol-classification.tsv`. Internal helpers should stay hidden
unless translated compatibility requires a broader static archive surface.
Keep the local deviation inventory in `../UPSTREAM_DIFF.md` current whenever an
upstream sync, ABI change, platform change, or Rust-only public surface change
lands.

## Security Updates

Security-relevant upstream changes to allocation, free, realloc, page ownership,
abandoned-page reclaim, guarded allocation, secure encoding, platform
primitives, or ABI behavior should be treated as release-blocking until parity
evidence is refreshed.

## Evidence Updates

An upstream sync must update evidence in the same branch:

- `../UPSTREAM_DIFF.md`: baseline version, deliberate local differences,
  limitations to recheck, and the upstream sync checklist.
- `PARITY.md`: supported parity matrix, C baseline header hashes, unresolved
  baseline mismatches, intentional non-parity decisions, vendored C test
  coverage, and signoff commands.
- ABI manifests under `docs/abi-manifests/`: regenerate with
  `scripts/check-public-symbols.py --write-dynamic-manifests` and verify with
  `scripts/check-abi-symbols.sh`.
- `BENCHMARK_RESULTS.md`: benchmark command lines, environment evidence,
  feature set, artifact mode, geomean Rust/C ratios, noisy scenarios, and
  release-blocking regressions when upstream touches hot paths or allocator-wide
  behavior.
- `docs/abi.md`, `docs/symbol-versioning.md`, `docs/platforms.md`,
  `docs/performance.md`, `docs/limitations.md`, and release notes when the sync
  changes guarantees, unsupported cases, artifact modes, symbol policy, target
  status, or performance conclusions.
