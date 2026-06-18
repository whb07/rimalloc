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
scripts/check-ffi-boundary.py
```

This check fails if a public-header symbol is missing or if an extra relevant
`mi_*`, `_mi_*`, malloc-family, or C++ operator symbol is exported without a
classification.
The FFI boundary check classifies source files containing export attributes as
C ABI, malloc/C++ override, runtime boundary, or internal-classified exception,
and verifies that the Linux linker version script keeps the expected public
symbol patterns global.

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

## Module Taxonomy

Current and target ownership are:

| Layer | Current owner | Responsibility |
| --- | --- | --- |
| Safe Rust facade | `src/lib.rs`, `src/safe.rs` | `MiMalloc`, `Allocation`, and Rust error/status types. |
| C header declarations and layouts | `src/abi.rs` exposed through `raw::abi` and `raw::mimalloc` | Public `mi_*` declarations, `repr(C)` layouts, C-compatible aliases, constants, and callback type aliases mirrored from public headers. |
| Exported C ABI implementations | existing implementation files approved by `scripts/check-ffi-boundary.py` | `#[unsafe(no_mangle)]`, `#[unsafe(export_name = ...)]`, naked ABI shims, and public assembly aliases. |
| Internal compatibility aliases | `src/mimalloc.rs` and selected translated helpers | C-shaped helper paths used by the mechanical translation but not a public Rust API. |
| Allocator internals | private modules such as `alloc`, `free`, `heap`, `page`, `arena`, `os`, and `threadlocal` | Implementation logic and typed invariant witnesses. |

The final raw layout should keep public C declarations in `src/abi.rs` and
expose them through `raw::abi`/`raw::mimalloc`. Default raw modules that exist
only for implementation or test convenience are migration targets for
`docs/raw-facade-migration.md`; they should be removed from the default facade
or placed behind an explicit unstable raw-internals feature.

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
