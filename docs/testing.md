# Testing And Hardening Policy

This repository uses several complementary test classes. A change does not need
to run every expensive class every time, but release claims must map to the
right evidence.

## Unit And Integration Tests

Default Rust tests cover translated API behavior, release-shape audits, feature
contracts, public ABI assumptions, and deterministic property-style allocation
sequences:

```sh
cargo test --quiet
cargo test --quiet --test property_alloc_sequences
cargo test --quiet --test fuzz_alloc_sequences
```

The property sequence test is intentionally deterministic. It exercises bounded
random malloc, calloc, aligned allocation, heap allocation, realloc, free,
usable-size, zero-fill, alignment, preservation, and heap-containment
properties without requiring an external fuzzing engine.

`tests/fuzz_alloc_sequences.rs` adds a byte-stream operation interpreter over a
fixed corpus. It covers malloc, zalloc, calloc, aligned allocation, heap
allocation, heap-aligned allocation, realloc, rezalloc, collect, null realloc,
overflow-sized request rejection, usable-size lower bounds, zero-fill,
alignment, preservation, and heap ownership. The interpreter is deterministic
in normal tests but keeps the operation model close to a future cargo-fuzz
target.

## Safe API / Raw Facade Split Checks

Use these checks when changing the crate-root facade, `raw`, FFI declarations,
or documentation that describes the split:

```sh
scripts/check-api-inventory.py
scripts/check-safe-boundary.py
scripts/check-raw-facade.sh
scripts/check-ffi-boundary.py
scripts/check-docs.sh
```

`scripts/check-api-inventory.py` verifies the crate-root safe API, default raw
modules, unstable raw-internals modules, and public inventory docs.
`scripts/check-safe-boundary.py` verifies that safe facade unsafe blocks and
unsafe impls are tagged and that safe APIs do not expose public unsafe items.
`scripts/check-raw-facade.sh` verifies that default `raw` does not re-export
implementation modules and that tests/examples/benches gate raw internals with
`unstable-raw-internals`.

## Sanitizers

AddressSanitizer is available through:

```sh
scripts/check-sanitizers.sh
```

The script uses a nightly toolchain, `-Zsanitizer=address`, `-Zbuild-std`, and
the `std` feature. Sanitizer runs are expected to be Linux x86_64 GNU evidence
unless the report says otherwise. TSan and UBSan are not currently part of the
maintained signoff set; claims about those tools require adding explicit scripts
and documenting known false positives.
Maintainers can also run the manual GitHub Actions hardening workflow for ASan
evidence.

`track-asan` is allocator tracking instrumentation, not a substitute for running
the sanitizer toolchain. `track-asan` and `track-valgrind` are mutually
exclusive feature modes.

## Valgrind

Valgrind tracking parity is checked by:

```sh
scripts/check-valgrind-tracking-parity.py
```

This check builds C and Rust probes and compares the tracked reports. It
requires `valgrind` on the host. The normal Rust test suite does not imply
Valgrind evidence.
Maintainers can also run the manual GitHub Actions hardening workflow for
Valgrind tracking evidence.

## Differential Tests Against C mimalloc

Behavioral parity against the vendored mimalloc 3.3.2 C baseline is covered by
the scripts listed in `PARITY.md`, especially:

```sh
scripts/check-behavioral-parity.py
scripts/check-cxx-operator-new-parity.py
scripts/check-secure-mode-parity.py
scripts/check-guarded-mode-parity.py
scripts/check-asan-tracking-parity.py
scripts/check-valgrind-tracking-parity.py
```

These checks compare null behavior, usable-size behavior, alignment, realloc
preservation, heap APIs, options, stats, feature modes, C++ overrides, and
platform-specific behavior where the current supported matrix claims parity.
Known unresolved baseline mismatches remain documented in `PARITY.md`.

## Fuzzing Policy

The current repository has deterministic property and byte-stream fuzz-corpus
tests but does not yet claim a maintained libFuzzer, AFL, or cargo-fuzz target.
A future external fuzz target should generate operation sequences over:

- malloc, calloc, zalloc, realloc, aligned allocation, heap allocation, collect,
  and free,
- nulls, zero sizes, overflow-sized requests, small sizes, large sizes, and
  alignment classes,
- per-heap ownership, cross-thread free, realloc preservation, zeroing,
  usable-size lower bounds, stats consistency, and feature-mode diagnostics.

Fuzz reproducers that trigger allocator misuse must classify whether the input
is valid API use, documented C-compatible abort behavior, or intentionally
unsupported invalid/double-free behavior.

## Miri

Miri is limited to pure Rust models and small library tests. The crate keeps
core-type unit tests for layout math, scalar domains, page owner/flag packing,
and thread-free word packing so those invariants can run without exercising the
full OS allocator:

```sh
scripts/check-miri.sh
```

Full OS allocator behavior is not expected to run under Miri until the relevant
subsystems have model-only tests.
