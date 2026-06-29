# Platform Support

The maintained target for this port is currently Linux x86_64 GNU.

## Tier 1

### Linux x86_64 GNU

Maintained for local development and release evidence.

Assumptions:

- ELF dynamic linking and Linux symbol visibility.
- pthread-compatible locking and TLS behavior.
- mmap, munmap, mprotect, madvise, getrandom or `/dev/urandom`, and libc errno.
- page-size and huge-page behavior matching the translated upstream Linux path.
- benchmark evidence is gathered on this tier unless a report says otherwise.

## Source-Present But Not Maintained Yet

The source tree still contains translated platform code for macOS, Windows,
WASI, Emscripten, and other Unix-like targets. Those targets are not claimed as
maintained Rust targets until they have parity tests, ABI/layout evidence,
platform-specific CI or local verification, and documented benchmark/security
coverage.

CI may run smoke validation for Linux aarch64, macOS, and Windows. Those jobs
check that the source keeps compiling and basic allocator tests run, but they do
not by themselves promote those platforms to maintained targets.

## no_std

Linux non-test builds use `no_std` when `std` is not enabled. The exact crate
attribute is:

```rust
all(
    not(test),
    target_os = "linux",
    not(feature = "std")
)
```

The `std` feature is intended for tests, examples, docs, tooling, and
benchmarks.

## Build Tools

`build.rs` intentionally invokes the platform C compiler, C++ compiler, and
archiver directly through `CC`, `CXX`, and `AR` environment variables, defaulting
to `cc`, `c++`, and `ar`. This keeps the C/C++ shim build behavior explicit for
ABI interposition work. `scripts/check-build-tools.sh` verifies that missing
tools fail with actionable remediation hints.
