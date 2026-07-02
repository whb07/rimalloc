# No-Allocation Critical Path Audit

Scope: allocator-internal paths that must not allocate through Rust's global
allocator while servicing mimalloc allocation, free, realloc, heap, theap, page,
arena, subprocess, and dynamic TLS operations.

The supported Linux x86_64 default matrix still permits mimalloc-owned
allocation in these paths: user blocks go through mimalloc pages, metadata goes
through `_mi_meta_zalloc`/`_mi_meta_free` or mimalloc page/arena allocation, and
dynamic TLS storage follows the C baseline's `mi_rezalloc`,
`mi_zalloc_aligned`, and `mi_free` behavior.

## Reviewed Critical Files

- `src/allocator/alloc.rs`, `src/allocator/alloc/public_malloc.rs`,
  `src/allocator/alloc/zeroed.rs`, `src/allocator/aligned.rs`, and
  `src/allocator/free.rs`: allocation, public malloc/heap malloc, zeroed/calloc,
  realloc, aligned allocation, and free paths use mimalloc page/theap helpers.
  Debug-only invalid-pointer diagnostics may format borrowed static C strings
  through `format_args!`, but the default supported path has `MI_DEBUG=0` and
  does not allocate through Rust's global allocator.
- `src/allocator/page.rs`, `src/allocator/page/deferred.rs`,
  `src/allocator/page/flags.rs`, `src/allocator/page/queues.rs`, `src/arena.rs`,
  `src/allocator/arena/os_memory.rs`, `src/allocator/arena/purge.rs`, and
  `src/arena_meta.rs`: page extension, free-list, deferred-free callback, page
  flag/owner access, queue link mutation/validation, arena reserve/manage-memory,
  purge/free dispatch, huge-object, and metadata allocation paths use OS, arena,
  or mimalloc metadata helpers. They do not use `Box`, `Vec`, `String`,
  `format!`, `CString`, or `std::alloc`.
- `src/heap.rs`, `src/theap.rs`, `src/init.rs`, and `src/threadlocal.rs`: heap,
  theap, subprocess, process/thread, and dynamic TLS metadata now avoid Rust
  heap allocation. Dynamic subprocess metadata uses `_mi_meta_zalloc` and
  `_mi_meta_free`; dynamic TLS tables use mimalloc's `mi_rezalloc`,
  `mi_zalloc_aligned`, and `mi_free`, matching `mimalloc-3.3.2/src/threadlocal.c`.
  Current Rust module paths for those lifecycle/TLS checks include
  `src/allocator/init.rs` and `src/allocator/threadlocal.rs`.
- `src/internal/bitmap.rs`, `src/internal/bitmap/bfield.rs`,
  `src/internal/bitmap/chunk_bits.rs`, `src/allocator/page_map.rs`,
  `src/internal/mod.rs`, `src/internal/page_location.rs`,
  `src/internal/page_metadata.rs`, and `src/platform/os.rs`: shared metadata
  helpers operate on caller-provided or OS/mimalloc-owned memory and do not
  allocate through Rust's global allocator.

## Non-Critical Formatting And Platform Helpers

`src/stats.rs`, `src/options.rs`, `src/libc.rs`, and platform-specific
inspection helpers contain `String`, `format!`, `CString`, or owned environment
string handling for output, JSON, diagnostics, and OS information collection.
Those calls are outside the core allocation/free critical path. They remain
covered by output/error callback reentrancy tests and by the public behavioral
parity suite, but they are not claimed to be allocation-free helpers.

## Runtime Guard

`tests/no_allocation_critical_paths.rs` installs a detecting Rust global
allocator for that test binary and records any global allocation, reallocation,
or deallocation while representative critical mimalloc APIs execute:

- `mi_malloc`/`mi_free`
- `mi_malloc_small`/`mi_free_small`
- `mi_malloc_aligned`/`mi_free`
- `mi_calloc`/`mi_free`
- `mi_rezalloc`/`mi_free`
- `mi_zalloc_aligned`/`mi_free`
- `mi_heap_new`/`mi_heap_delete`

This is intentionally a recursion detector for Rust global allocator use, not a
ban on mimalloc allocating pages or metadata for its own operation.

## Guardrail

`NO_ALLOCATION_CRITICAL_PATHS.json` maps public critical entrypoints to the
source files that own them: `mi_malloc`, `mi_malloc_small`, `mi_calloc`,
`mi_rezalloc`, `mi_heap_malloc`, `mi_zalloc_aligned`, `mi_free`,
`mi_free_small`, and `mi_heap_delete`. `scripts/check-no-allocation-audit.py`
checks this manifest, this audit, the runtime guard test, the reviewed critical
source files, regex smoke-test allocation constructs, and direct calls from
manifest entrypoints to known allocating helpers. The full validation for this
audit is:

```sh
python3 scripts/check-no-allocation-audit.py
cargo test --test no_allocation_critical_paths
```
