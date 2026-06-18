# ABI Policy

The crate builds a `cdylib` that exports mimalloc-compatible C ABI symbols and
an `rlib` with an explicit Rust `raw` facade. The raw Rust facade lives under
`mechanical_mimalloc::raw` and mirrors the C shape for tests, benchmarks, and
low-level integration.

## Compatibility Domains

- C exported symbols and calling conventions.
- `repr(C)` raw layouts and field offsets.
- Rust `raw` facade paths.
- Safe Rust facade paths.
- Feature-dependent layout and behavior.
- MSRV and target support.

Changing any of these domains requires an explicit changelog entry and matching
tests or audit evidence.

## Expected Export Surface

The expected exported C surface is derived from:

- `mimalloc-3.3.2/include/mimalloc.h`
- `mimalloc-3.3.2/include/mimalloc-stats.h`
- malloc-family override symbols used by upstream mimalloc on Linux
- Itanium C++ operator new/delete override symbols
- classified Rust translation extensions in
  `symbols/public-symbol-classification.tsv`

Run:

```sh
scripts/check-abi-symbols.sh
```

This check fails if a public-header symbol is missing or if an extra relevant
`mi_*`, `_mi_*`, malloc-family, or C++ operator symbol is exported without a
classification.

## Hidden Symbols

On Linux cdylib builds, `build.rs` emits a linker version script that marks the
explicit mimalloc/malloc/C++ override surface global and all other symbols
local. Internal translated helpers should stay hidden unless they are
deliberately classified.

For static archive audits, some internal `_mi_*` helpers remain visible because
the mechanical translation still uses C-shaped linkage internally. These are
classified as `internal-export-required` and are not stable public API.

`tests/release_fast_path_audit.rs` also guards known internal helpers against
regaining `#[unsafe(no_mangle)]`.

## Layout and Constant Compatibility

Run:

```sh
scripts/check-layout.sh
```

This check compares Rust declarations against the vendored C headers for public
layout size, alignment, field offsets, public constants, option values, stats
initialization behavior, and release-fast-path source invariants.

Public `repr(C)` structs and C-compatible aliases are compatibility-sensitive.
Changing size, alignment, offsets, enum discriminants, callback signatures, or
feature-dependent layout requires ABI notes and matching test updates.

## Raw Rust Facade

`mechanical_mimalloc::raw` is the Rust path for C-shaped constants, callbacks,
layouts, and functions. It is intentionally separate from the crate root:

- crate root: safe `MiMalloc` facade and `raw`
- `raw::mimalloc` and `raw::abi`: public C header mirror
- `raw::{alloc, free, heap, stats, options, ...}`: compatibility modules for
  tests, examples, and low-level users

Most raw functions are unsafe because pointer validity, allocation origin,
alignment, lifetime, nullability, and threading are caller obligations.

## Callback ABI

Callback types mirror upstream mimalloc C callback signatures. Callbacks must
not unwind across the C ABI boundary. Callers are responsible for ensuring
callback function pointers and callback arguments remain valid for the duration
of the call that receives them.

## Null, Errno, and Error Behavior

Raw allocation functions preserve C-style behavior:

- allocation failure returns null where upstream returns null;
- free accepts null;
- functions with out-pointers require valid writable storage unless the C API
  explicitly allows null;
- errno-compatible behavior follows the C-shaped API and parity tests.

Safe Rust APIs must not expose C null/error conventions directly. New safe
wrappers require explicit Rust error/status types or a documented infallible
contract.

## Panic and Unwind Policy

Allocator ABI entry points must not unwind across C boundaries. Production
Linux release cdylib builds use panic abort behavior, and audit tests check the
no-std/panic setup.

## C++ Operator New/Delete

Linux non-Windows builds include C++ operator new/delete override shims matching
the upstream mimalloc override surface. The exported Itanium symbols are audited
by `scripts/check-abi-symbols.sh`; behavior is covered by the C++ parity audit
script and release-fast-path source checks.

## Feature-Dependent ABI

Feature flags can affect layout or behavior. `secure`, `guarded`,
`track-valgrind`, and `track-asan` must be checked independently where relevant.
`track-valgrind` and `track-asan` are mutually exclusive.

Before claiming ABI compatibility for a feature set, run the relevant feature
sweep and ABI/layout checks for that feature set or document why the feature
does not affect public layout or symbols.
