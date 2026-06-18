# ABI Policy

The crate builds a `cdylib` that exports mimalloc-compatible C ABI symbols.
The raw Rust facade lives under `mechanical_mimalloc::raw` and mirrors the C
shape for tests, benchmarks, and low-level integration.

## Compatibility Domains

- C exported symbols and calling conventions.
- `repr(C)` raw layouts and field offsets.
- Rust `raw` facade paths.
- Safe Rust facade paths.
- Feature-dependent layout and behavior.
- MSRV and target support.

Changing any of these domains requires an explicit changelog entry and matching
tests or audit evidence.

## Panic and Unwind Policy

Allocator ABI entry points must not unwind across C boundaries. Production
Linux release cdylib builds use panic abort behavior, and audit tests check the
no-std/panic setup.

