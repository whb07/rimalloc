# Hardening Threat Model

This document defines what the allocator hardening features are expected to
detect or mitigate, and what remains outside their guarantee. It applies to the
maintained release target and documented Rust/C ABI surfaces.

## Baseline Assumptions

The allocator assumes well-formed safe Rust callers, documented raw/C ABI
callers, and a process that has not already granted arbitrary memory write
capability to an attacker. Unsafe raw callers must uphold pointer provenance,
allocation origin, alignment, lifetime, synchronization, and callback contracts.

Hardening features are defense-in-depth. They may turn some corruptions into
null returns, aborts, assertions, guard-page faults, or rejected free paths, but
they do not make invalid unsafe calls sound.

## `secure`

The `secure` feature is expected to mitigate or detect selected allocator
metadata corruptions through secure-mode padding, canaries, encoded free lists,
and secure/release randomized address hints where the platform supports them.

Expected coverage:

- detect some buffer overruns that reach secure padding metadata,
- make free-list pointer manipulation harder through encoded links,
- reject or abort on selected corrupted free-list or padding states,
- reduce predictability of selected OS allocation addresses and guard metadata
  placement when platform entropy is available.

Non-goals:

- complete temporal memory safety,
- guaranteed detection of every double free, invalid free, or interior free,
- cryptographic protection for allocator metadata,
- protection after an attacker can write arbitrary allocator metadata,
- compatibility with every unsupported target's entropy and memory-protection
  primitives.

## `guarded`

The `guarded` feature is expected to detect selected out-of-bounds and
use-after-free patterns by sampling allocations into guarded allocation
placements. Sampling is controlled by guarded size bounds, sample rate, sample
count, and guarded options.

Expected coverage:

- catch some overflows or underflows around sampled guarded allocations,
- catch some use-after-free accesses that fault after guarded deallocation,
- mark guarded blocks so free validation can route them through guarded cleanup,
- provide release evidence for guarded allocation paths through feature and ABI
  checks.

Non-goals:

- coverage for every allocation,
- deterministic detection unless the guarded configuration makes sampling
  deterministic for the reproducer,
- protection for allocations outside configured size bounds,
- protection from callers that forge guarded tags or mutate allocator metadata.

## Out Of Scope For Malicious Unsafe Callers

The allocator does not attempt to make these caller behaviors safe:

- forged pointers, foreign allocations, or interior pointers passed as owned
  allocation starts,
- use-after-free, double-free, or realloc-after-free by raw/C ABI callers,
- unsynchronized mutation of allocator state through raw pointers,
- invalid layouts, alignments, callback function pointers, or callback user data,
- direct writes to pages, heaps, arenas, free lists, padding, or guard metadata,
- arbitrary process-memory write, code execution, or data-race capability before
  allocator entry.

Reports for these cases are still useful when behavior diverges from C mimalloc
3.3.2, affects a safe Rust API, crosses an FFI unwind boundary, or exposes a
bug in a documented hardening mode.

## Entropy, Canaries, And Metadata Corruption

Entropy for hardening keys, canaries, random cookies, and randomized address
hints must come from the platform primitive path or the documented fallback path
for the target. A supported target must treat predictable entropy, repeated
canaries, or failed random initialization as security-sensitive.

Canaries and encoded free-list links assume the hardening keys remain private
inside the process and that corruption is partial or accidental enough for the
checks to observe it. They are not a cryptographic boundary against a caller
that can read keys, rewrite metadata consistently, or control both user memory
and allocator metadata.

Allocator metadata corruption is in scope when well-formed safe Rust or
documented C ABI use can corrupt allocator-owned metadata. Metadata corruption
caused only by malicious unsafe callers is out of scope unless `secure` or
`guarded` explicitly claims to detect that pattern.

## Report Requirements

Hardening reports should include:

- exact feature set and artifact mode,
- target platform, Rust toolchain, and entropy constraints,
- whether `secure`, `guarded`, `track-valgrind`, or `track-asan` was enabled,
- guarded size bounds, sample rate, sample seed, and reproducer determinism when
  guarded allocation is involved,
- sanitizer, Valgrind, Miri, debugger, or core dump evidence when available,
- whether C mimalloc 3.3.2 reproduces the same behavior.
