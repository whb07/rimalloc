# Upstream Synchronization Policy

This repository tracks mimalloc 3.3.2 as the upstream behavioral and ABI
baseline. Deliberate local differences and sync-time recheck items are recorded
in `../UPSTREAM_DIFF.md`.

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
