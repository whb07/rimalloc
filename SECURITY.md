# Security Policy

## Supported Versions

This project is pre-release. Security fixes apply to the current `master`
branch unless a release branch is explicitly created.

## Reporting

Please report suspected memory-safety, allocator-correctness, or security issues
privately to the repository maintainers. Include:

- target platform and Rust version,
- enabled features,
- a minimized reproducer,
- whether the issue requires `secure`, `guarded`, `track-valgrind`, or
  `track-asan`,
- observed crash, corruption, leak, or invariant failure.

## Scope

Security-sensitive areas include allocation/free/realloc behavior, cross-thread
free lists, page ownership, abandoned-page reclaim, padding/canary checks,
guarded allocation metadata, secure-mode encoding, platform primitives, and C
ABI compatibility.

The `secure` and `guarded` features are hardening aids, not complete protection
against arbitrary memory corruption or malicious unsafe callers.

