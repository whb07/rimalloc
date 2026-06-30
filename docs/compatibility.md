# Compatibility Policy

This repository is pre-release, but compatibility domains are tracked now so
accidental public commitments do not leak out of the mechanical translation.

This crate provides safe Rust allocation, heap, options, statistics, and
process-info facades over a translated mimalloc allocator. It can also build an
explicit mimalloc-compatible C ABI/interposition artifact. Borrowed callback
guards live under the explicit unsafe-adjacent Rust callback namespace.

## Public API Tiers

### Tier S: Safe Rust API

The crate-root safe API is the Rust SemVer surface. It contains `MiMalloc`,
owned allocation wrappers, heap wrappers, supported options, non-printing stats
snapshots, process information, and the typed error/status types listed in
`docs/api-inventory.md`.

Compatibility-sensitive changes include removing or renaming public safe items,
changing public method signatures, changing trait implementations, changing
documented safety or ownership behavior, and changing error variants or their
meaning.

### Tier R: Raw Rust Facade

`mechanical_mimalloc::raw` is an explicit C-shaped compatibility facade. It is
not a safe Rust abstraction and is not yet promised as a stable Rust SemVer
surface. Changes must still be reviewed because tests, examples, benchmarks,
and low-level users may depend on it.

The default raw facade is intentionally narrow: root `raw::mi_*`, `raw::abi`,
`raw::mimalloc`, and `raw::headers`. Translated implementation modules are
crate-private and are not exposed as raw Rust module paths.

Raw facade changes require updating `docs/api-inventory.md`, ABI/layout checks,
and any affected examples or parity tests.

### Tier U: Unsafe Rust Convenience API

`mechanical_mimalloc::unsafe_callbacks` contains borrowed callback guards that
are public unsafe Rust API but not Tier S safe facade API and not Tier R/C-shaped
raw API. Changes to these guards must be reviewed for Rust source
compatibility, documented safety obligations, process-global callback behavior,
panic containment, and API inventory drift.

Tier U currently contains `unsafe_callbacks::ErrorCallbackGuard` and
`unsafe_callbacks::DeferredFreeCallbackGuard`.

### Tier C: C ABI

The opt-in `cdylib` artifact exports mimalloc-compatible C ABI symbols when
built with the explicit interposition feature set. Compatibility-sensitive
changes include exported symbol additions/removals, calling convention changes,
panic/unwind behavior, errno/null behavior, callback ABI changes, and C++
operator new/delete override behavior.

`staticlib` artifacts are preview-only in the current release policy.
Repository tests may build them for parity and audit coverage, but their exact
archive symbol contract, link-collision behavior, and release manifests are not
frozen as a production FFI compatibility surface yet.

Use:

```sh
scripts/check-abi-symbols.sh
scripts/check-c-link-public-api.py
```

### Tier I: Internal Implementation

Private allocator implementation modules are not public API. Any new `pub`
item outside the crate-root facade or documented `raw` allowlist is
release-blocking until classified. Any new `#[unsafe(no_mangle)]`,
`#[unsafe(export_name = ...)]`, naked ABI shim, or `global_asm!` export outside
approved FFI boundary files is release-blocking until classified.
`types` is a private design surface for strict internal wrappers and invariant
witnesses; it is not part of the safe API, raw facade, or C ABI.

Use:

```sh
scripts/check-api-inventory.py
scripts/check-ffi-boundary.py
```

## Other Compatibility Domains

### Raw Layout

`repr(C)` layouts, constants, enum discriminants, offsets, sizes, and alignments
are C compatibility surface. Layout changes require a C baseline comparison and
must be documented as intentional.

Use:

```sh
scripts/check-layout.sh
```

### Features

Feature behavior is compatibility-sensitive. Defaults are intentionally empty.
`std`, `bench`, `compare`, `bench-diagnostics`, `secure`, `guarded`,
`track-valgrind`, `track-asan`, `c-header-macros`, `c-abi`, `cxx-overrides`,
`tls-shim`, and `interpose` are documented in `docs/features.md`.

When `track-valgrind` and `track-asan` are both enabled, Valgrind mode takes
precedence. New features should be additive unless a compile-time conflict is
unavoidable and explicitly checked.

### MSRV And Tooling

The MSRV is the `rust-version` in `Cargo.toml` and the pinned toolchain in
`rust-toolchain.toml`. Raising MSRV is compatibility-sensitive and must update
release notes, reproducibility docs, and local verification expectations.

### Supported Targets

Supported target tiers are documented in `docs/platforms.md`. Dropping or
weakening a maintained target is compatibility-sensitive. Source-present
platform code is not a maintained target without tier evidence.

### Safety Preconditions

Tightening or loosening documented safety preconditions is compatibility-
sensitive. Safe Rust APIs must not gain new caller obligations. Raw APIs may
inherit C preconditions, but changes to documented ownership, aliasing,
alignment, lifetime, threading, invalid-free, or unwind behavior must be
reviewed as compatibility changes.

### Performance

Performance is not SemVer in the Cargo sense, but allocator users treat it as a
release contract. Hot-path and allocator-wide changes must follow
`docs/performance.md`, including threshold, noise, and benchmark-evidence rules.

## Release Checklist

Before claiming a compatibility checkpoint:

1. Run `scripts/check-api-inventory.py`.
2. Run `scripts/check-features.sh`.
3. Run `scripts/check-layout.sh`.
4. Run `scripts/check-abi-symbols.sh`.
5. Run `scripts/check-package.sh`.
6. Run benchmark checks required by `docs/performance.md`.
7. Update `CHANGELOG.md` and affected policy docs.

## Classification

- `breaking`: removes or changes a promised safe Rust API, C ABI symbol/layout,
  maintained feature behavior, MSRV, maintained platform, or safety
  precondition.
- `compatible`: adds documented safe API or raw/C ABI surface without changing
  existing behavior, and with tests/docs.
- `internal`: changes private implementation details without affecting any
  compatibility domain.
- `performance-sensitive`: may be source-compatible but requires benchmark
  evidence before release.
