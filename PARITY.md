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

## Rust-Only Public Export Safety Contracts

These exports are not public C mimalloc 3.3.2 API. They are excluded from the
C parity allowlist by `symbols/public-symbol-classification.tsv`, and
`scripts/check-public-symbols.py` fails on any unclassified extra export.

The Rust-only exports are compatibility liabilities, not stable public ABI. New
callers should prefer the C mimalloc API unless a contract below explicitly
covers the required ownership, aliasing, thread, heap, and callback behavior.

| Classification | Exports | Contract |
|---|---|---|
| Internal translation exports | `_mi_*` | These are unstable allocator internals kept exported for translated code paths. Callers must not treat them as public API. Any pointer, heap, arena, page, segment, or subprocess handle passed to them must originate from this allocator, remain live for the full call, and obey the same thread and aliasing restrictions as the corresponding internal C path. Callback pointers must use the C ABI, remain valid for the call or registration lifetime, and must not unwind across FFI. |
| Arena inspection helpers | `mi_arena_from_index`, `mi_arena_id_from_arena`, `mi_arenas_get_count` | Arena identifiers and arena pointers are opaque, process-local observations. These helpers do not transfer ownership. A returned identifier may only be passed back to this allocator while the referenced arena remains live; callers must not dereference stale or fabricated arena pointers. |
| Heap, stats, and ownership helpers | `mi_heap_check_owned`, `mi_heap_stats_merge_to_main`, `mi_malloc_size_checked`, `mi_stats_get_json_from` | Heap and stats pointers must be valid objects from this allocator. Calls must not race with unsupported heap or stats mutation from another thread. JSON buffers returned from allocator-owned paths must be released with `mi_free` when ownership is transferred to the caller. |
| Thread-local heap allocation helpers | `mi_theap_realpath`, `mi_theap_umalloc`, `mi_uzalloc` | `mi_theap_t*` values must refer to live thread-local heaps from this allocator and must remain on a thread where the underlying theap operation is valid. Returned allocations are mimalloc-owned and must be released with `mi_free` or the matching public mimalloc free API. Writable out-parameters must be valid for writes unless the specific helper documents that null is accepted. |
| Subprocess alias | `mi_subproc_delete` | The subprocess id must come from this allocator. The function aliases `mi_subproc_destroy`; callers must not destroy the main/current subprocess or any subprocess still used by live heaps or threads. |
| OS and TLS platform helpers | `mi_os_alloc`, `mi_os_alloc_aligned`, `mi_os_alloc_aligned_allow_large`, `mi_os_commit`, `mi_os_decommit`, `mi_os_free`, `mi_tls_raw_index_cached`, `mi_tls_raw_index_default` | These are platform-facing allocator internals. Pointers, sizes, alignment, commit state, and ownership must satisfy the allocator OS facade requirements. Memory returned by these helpers must be released only through the matching helper path, and the TLS indices are allocator implementation details. |

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
