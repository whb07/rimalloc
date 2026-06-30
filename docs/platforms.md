# Platform Support

The maintained target for this port is currently Linux x86_64 GNU.

Production status:

- Supported: Tier S safe Rust facade, Tier U unsafe callback guards, and Tier C
  dynamic C ABI release artifacts `c-abi-cdylib` and `interpose-cdylib`.
- Preview-only: staticlib release artifacts `c-abi-static-preview` and
  `interpose-static-preview`.
- Unstable: Tier R raw Rust facade path stability, translated implementation
  modules, internal strict types, extension ABI, and static archive internal
  symbols.
- Unsupported: source-present non-maintained platforms are not release artifact
  targets.
- Maintained target: Linux x86_64 GNU only.

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

The source tree still contains translated platform code for Linux aarch64,
macOS, Windows, WASI, Emscripten, and other Unix-like targets. Those targets
are source-present only unless this document explicitly promotes them. They are
not claimed as maintained Rust targets until they have parity tests, ABI/layout
evidence, platform-specific CI or local verification, and documented
benchmark/security coverage.

CI may run smoke validation for Linux aarch64, macOS, and Windows. Those jobs
check that the source keeps compiling and basic allocator tests run, but they do
not by themselves promote those platforms to maintained targets.
The current workflow labels those jobs as:

- `Linux aarch64 smoke validation`
- `macOS smoke validation`
- `Windows smoke validation`

## Unsupported Target Reports

Release notes must not market source-present or smoke-only targets as
maintained. Use this language unless a platform has completed the promotion
checklist:

```text
Linux x86_64 GNU is the only maintained target. Linux aarch64, macOS, Windows,
WASI, Emscripten, and other source-present targets are smoke-tested or
source-present only unless promoted in docs/platforms.md. Reports on those
targets are triaged as portability issues and are not release blockers unless
they reproduce on Linux x86_64 GNU or expose a cross-platform safety/security
bug.
```

## Platform Promotion Checklist

A source-present platform can be promoted to maintained target status only when
the release owner records all of the following evidence in this document or a
linked release signoff artifact:

- Compile and test CI for the platform, including the default library build and
  the relevant feature combinations for that target.
- ABI/layout checks where the platform has a C ABI story, including
  `scripts/check-layout.sh` or a platform-specific equivalent that verifies
  public `repr(C)` layouts, constants, callback signatures, and symbol
  expectations.
- C and C++ link tests where FFI artifacts are claimed, including consumer
  coverage equivalent to `scripts/check-release-artifact-consumer.py`,
  `scripts/check-release-link-metadata.py`,
  `scripts/check-release-dlopen.py`,
  `scripts/check-release-ld-preload.py`, and
  `scripts/check-release-cxx-artifacts.py` when those modes apply.
- Sanitizer or equivalent hardening evidence, such as ASan/TSan, Miri,
  Valgrind, platform-native memory diagnostics, or a written explanation for
  why the platform cannot run those tools with comparable coverage.
- Platform primitive audit covering allocator OS calls, TLS, locking, entropy,
  stderr/output routing, page-size discovery, huge-page behavior, and dynamic
  loader behavior for that platform.
- Benchmark evidence on the promoted platform, or an explicit performance
  caveat stating which workloads lack measurement and why promotion is still
  acceptable.
- Documented limitations covering unsupported features, smoke-only paths,
  required toolchains, known behavioral divergences, and release triage policy.

Until every checklist item is complete, the platform remains source-present or
smoke-tested only, not maintained.

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
