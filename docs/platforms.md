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

## no_std

Optimized Linux non-test builds use `no_std` when `std` is not enabled and
debug assertions are off. The exact crate attribute is:

```rust
all(
    not(test),
    target_os = "linux",
    not(debug_assertions),
    not(feature = "std")
)
```

The `std` feature is intended for tests, examples, docs, tooling, and
benchmarks.
