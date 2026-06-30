# Security Policy

## Supported Versions

This project is currently pre-release. Until the first published release,
security fixes apply to the current `master` branch only.

After published releases exist, security fixes apply to these lines:

| Release line | Security-fix status |
| --- | --- |
| Current `master` before a release is tagged | Receives fixes before the first release and between releases. |
| Latest published release line, such as `0.3.x` after `0.3.0` is published | Receives security patch releases. |
| Older release lines, such as `0.2.x` after `0.3.0` is published | Unsupported unless release notes explicitly announce extended support. |
| Release candidate or other pre-release branches | Receive fixes only until the final release for that version is published or the candidate is abandoned. |

There is no implicit long-term support line. If maintainers create one, the
release notes and this table must name the supported line, support window, and
security contact process.

Security fixes for a supported published line should ship as patch releases
when the fix can be backported without changing that line's documented API, ABI,
artifact, or target-support contract. If the fix cannot be backported safely,
maintainers must publish the advisory and release notes with the affected
versions, the first fixed version, and the required upgrade path.

Pre-release branches stop receiving security fixes when their final release is
published, when a newer release candidate supersedes them, or when maintainers
mark the candidate abandoned in release notes.

## Reporting

Please report suspected memory-safety, allocator-correctness, or security issues
privately through GitHub Security Advisories:

```text
https://github.com/whb07/mechanical_mimalloc/security/advisories/new
```

Do not file a public issue with vulnerability details before the report is
triaged. If GitHub private vulnerability reporting is unavailable, open a
minimal public issue that says only that you need a private security contact;
do not include a reproducer, crash details, exploit details, or private
environment data in that public issue.

Include:

- affected crate version, release tag, or commit SHA,
- target platform and Rust version,
- target triple and whether the report uses the Rust library, `c-abi` cdylib,
  `interpose` cdylib, preview static archive, or raw facade,
- enabled features and relevant environment variables,
- a minimized reproducer or private repository with reproduction steps,
- whether the issue requires `secure`, `guarded`, `track-valgrind`, or
  `track-asan`,
- observed crash, corruption, leak, invariant failure, or security impact,
- whether C mimalloc 3.3.2 reproduces the same behavior,
- sanitizer, Miri, Valgrind, debugger, core dump, or log output when available.

`docs/testing.md` defines the fuzz corpus and minimized reproducer policy,
including where public reproducers live, when corpus additions are required, how
to classify invalid-caller undefined behavior versus allocator bugs, and how
confirmed fuzz findings map back to unsafe-audit evidence.

## Response Timeline

The maintainer response targets are:

- Acknowledge receipt within 3 business days.
- Provide initial triage within 7 calendar days after enough reproduction
  information is available.
- Provide a severity/fix-plan update within 14 calendar days, or explain why
  more information is needed.
- Coordinate public disclosure through a GitHub Security Advisory, changelog
  entry, and release notes after a fix is available, unless the reporter and
  maintainers agree on a different timeline.

`docs/security-advisories.md` defines severity triage, CVE handling, embargoes,
coordinated disclosure, and changelog/advisory publication requirements.

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

`docs/hardening-threat-model.md` defines the detailed hardening threat model for
`secure`, `guarded`, malicious unsafe callers, entropy, canaries, and allocator
metadata corruption.

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
but unsupported targets are welcome. They are triaged as portability issues
unless one of these conditions holds:

- the issue reproduces on Linux x86_64 GNU,
- the issue exposes a cross-platform safety or security bug,
- the issue affects release artifact generation, symbol policy, or documented
  compatibility claims for the maintained target.

Unsupported-target reports that do not meet those conditions may still be fixed,
but they are not release blockers for the maintained target line.
