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

## Threat Model

This allocator must not introduce memory unsafety for well-formed Rust or C ABI
callers. It is still an allocator with unsafe raw entry points: malicious or
incorrect callers can pass invalid pointers, use freed memory, race on shared
objects, or violate alignment and ownership contracts. Those caller bugs are
outside the safe Rust model and may remain undefined behavior unless a specific
hardening mode detects them first.

Security-relevant reports include:

- allocator-owned memory corruption,
- use-after-free or double-free that is not handled like upstream mimalloc,
- wrong usable-size or alignment reporting,
- cross-thread free-list corruption,
- page ownership or abandoned-page reclaim corruption,
- C ABI panic/unwind across FFI,
- entropy, canary, or guarded-allocation failures,
- platform primitive behavior that can corrupt or disclose memory.

## Hardening Features

`secure` enables secure-mode padding and encoded free-list behavior where the
translation supports it. It is intended to make some metadata corruption and
invalid free-list manipulation easier to detect or harder to exploit.

`guarded` enables guarded allocation sampling. It is intended to catch selected
out-of-bounds or use-after-free patterns for sampled allocations.

These modes are hardening aids, not complete protection against arbitrary memory
corruption, data races, malicious unsafe callers, forged allocator metadata, or
all invalid/double-free cases.

## Invalid and Double Free Behavior

Default behavior follows the translated upstream mimalloc behavior and active
feature set. Invalid frees, double frees, interior frees, and foreign-pointer
frees are security-sensitive because behavior can vary by build mode, debug
assertions, secure/guarded configuration, and platform support. Reports should
include the exact feature set and whether the observed behavior differs from C
mimalloc 3.3.2.

## Randomness Assumptions

Entropy used for allocator hardening must come from the platform primitive path
or the documented fallback path for the target. Reports involving predictable
keys, repeated canaries, or failed random initialization should include OS,
architecture, sandbox/container constraints, and whether `secure` or `guarded`
was enabled.

## Unsupported Targets

Linux x86_64 GNU is the currently maintained target. Reports for source-present
but unsupported targets are welcome, but they may be triaged as platform
enablement work unless the issue also affects the maintained target.
