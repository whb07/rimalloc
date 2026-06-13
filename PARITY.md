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
mode, guarded mode, Valgrind tracking, ASAN integration, and allocator
override/interposition parity.

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
