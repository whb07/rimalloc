# Rust mimalloc 3.3.2 Parity Baseline

This document defines the initial parity scope for the Rust port against the
vendored C mimalloc 3.3.2 source tree.

## Supported Parity Matrix

The current claimed parity matrix is intentionally narrow. Later validations in
`TODO.md` must map to one or more rows here before a checkbox can be closed.

| Row | OS | Architecture | Pointer width | Endianness | libc/CRT | Rust profile | mimalloc mode |
|---:|---|---|---:|---|---|---|---|
| linux-x86_64-gnu-default-debug | Linux | x86_64 | 64 | little | glibc | dev | default allocator mode: `MI_DEBUG=0`, `MI_SECURE=0`, `MI_GUARDED=0`, tracking disabled |
| linux-x86_64-gnu-default-release | Linux | x86_64 | 64 | little | glibc | release | default allocator mode: `MI_DEBUG=0`, `MI_SECURE=0`, `MI_GUARDED=0`, tracking disabled |

Unsupported until added here with evidence: Windows, macOS, WASI,
Emscripten, non-x86_64 architectures, big-endian targets, debug mode, secure
mode, guarded mode, Valgrind tracking, sanitizer builds (ASAN/TSAN/UBSAN),
Windows ETW/event tracking, and non-Linux allocator override/interposition parity.

## Intentional Non-Parity Decisions

| Area | Affected APIs | Target/mode | User-visible behavior | Reason | Containment |
|---|---|---|---|---|---|
| C++ bad-allocation throwing from Rust | `mi_new`, `mi_new_n`, `mi_heap_alloc_new`, `mi_heap_alloc_new_n`, C++ global `operator new` overrides | Rust artifact linked into C++ callers on Unix | Successful allocations and installed `std::new_handler` callbacks are supported; if no handler can satisfy an allocation, Rust aborts instead of throwing `std::bad_alloc` from Rust across the C ABI. | C mimalloc only throws `std::bad_alloc` when that implementation is compiled as C++; the Rust implementation cannot safely originate a C++ exception across an `extern "C"` Rust boundary. | `scripts/check-behavioral-parity.py` covers successful `mi_new` allocation/free compatibility and installed-new-handler invocation. The remaining exception behavior keeps the `mi_new` and C++ override TODO items open. |

## C Baseline

The public API parity source of truth is the vendored mimalloc 3.3.2 C headers:

| Header | SHA-256 | FNV-1a 64 | Bytes |
|---|---|---:|---:|
| `mimalloc-3.3.2/include/mimalloc.h` | `869161433ab2d807a75752d0762accf5b68b6b24ce67e417af46bf181b9b6c72` | `0x1ecbc696d43439d7` | 47719 |
| `mimalloc-3.3.2/include/mimalloc-stats.h` | `5260df227ea1612b8678a06b063cc8f48e739b551554541cdeb4d430a7e257b3` | `0x2552612863535d3a` | 7349 |
| `mimalloc-3.3.2/include/mimalloc-new-delete.h` | `dd4f25cae53209d45d73f8e6a2b9c219e8fc7434d97f20eba9af1a8b850030fd` | `0x8a5aa49e0ef8c48a` | 4044 |

The header constants currently verified from the baseline are:

| Constant | Value |
|---|---:|
| `MI_MALLOC_VERSION` | 30302 |
| `MI_STAT_VERSION` | 5 |

Any change to these vendored headers must be reviewed as a baseline change
before Rust public API parity items can be considered complete.

## Signoff Command Set

Run this sequence from the repository root before declaring a parity checkpoint
complete for the currently supported matrix:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --release
scripts/check-public-abi.py
scripts/check-public-symbols.py
scripts/check-public-symbols.py --missing-only
scripts/check-c-link-public-api.py
scripts/check-behavioral-parity.py
```

Each command must exit successfully. The script checks compare the Rust artifact
against the vendored C mimalloc 3.3.2 headers and implementation; `cargo test`
and `cargo test --release` cover the two Rust profiles currently listed in the
supported parity matrix.
