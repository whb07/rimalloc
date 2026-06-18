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
plan exist. `SECURITY.md` defines the current reporting scope.
