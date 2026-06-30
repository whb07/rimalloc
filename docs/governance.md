# Governance

This project is pre-release. Maintainer responsibilities are intentionally
explicit because allocator changes can affect safety, ABI compatibility, and
performance.

## Review Expectations

- Unsafe changes need an unsafe-audit reviewer.
- C ABI changes need an ABI/layout reviewer.
- Hot-path or broad allocator changes need benchmark sign-off.
- Platform changes need platform-specific verification evidence.
- Upstream synchronization changes need a parity review.

## Ownership Roles

- Maintainers own triage, scope control, and final merge decisions.
- Unsafe reviewers own proof obligations for pointer validity, ownership,
  alignment, initialization, aliasing, concurrency, and unwind behavior.
- Release owners own the release checklist in `docs/release.md`.
- Security contact is the maintainer group reached through the private process
  in `SECURITY.md`.
- Upstream sync owners own baseline updates, `UPSTREAM_DIFF.md`, `PARITY.md`,
  and ABI/layout/performance rechecks for new mimalloc versions.
- Benchmark sign-off owners own benchmark scope, noise classification, and the
  no-regression conclusion recorded in `BENCHMARK_RESULTS.md`.

## Release Ownership

A release owner must confirm:

- public API inventory,
- C ABI symbol inventory,
- feature sweep,
- ABI/layout audit,
- package contents,
- benchmark evidence when relevant,
- changelog and policy docs.

## Security Ownership

Security-sensitive reports should be handled privately until a fix and advisory
plan exist. `SECURITY.md` defines the current reporting scope, and
`docs/security-advisories.md` defines severity triage, CVE handling, embargoes,
coordinated disclosure, and changelog/advisory publication requirements.
