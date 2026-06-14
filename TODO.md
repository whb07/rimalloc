# TODO: Rust mimalloc allocator parity with C mimalloc 3.3.2

Scope: actual allocator, runtime, page/arena, heap/theap/subprocess, mode, and
behavioral-test parity for the Rust implementation in this repository compared
to the vendored C implementation under `mimalloc-3.3.2`.

This pass intentionally excludes public exported-symbol-only parity, CMake/build
system parity, release packaging polish, and unrelated repository tooling. Those
can stay documented in `PARITY.md` and the symbol scripts, but they are not
blockers for the allocator-behavior checklist below.

Completion rule: every checkbox in this file must be closed, every intentional
allocator-behavior divergence must be documented with a compatibility rationale,
and the final behavioral test matrix must match C mimalloc 3.3.2 for the
supported targets in `PARITY.md`.

## Core Allocation Semantics

- [x] Match C guarded allocation dispatch for generic allocations. Validation: Rust `mi_theap_malloc_generic` performs the same `MI_GUARDED`, `huge_alignment == 0`, and `mi_theap_malloc_use_guarded` check as C before calling `_mi_malloc_generic`; C-vs-Rust guarded parity covers at least one request above `MI_SMALL_SIZE_MAX` such as 2048 bytes with guarded bounds including it, and verifies `malloc_guarded_count`, usable size, end-of-object guard alignment, and guard-page fault behavior.
- [x] Match C page commit-on-demand semantics. Validation: when `mi_option_page_commit_on_demand` makes a regular page initially uncommitted, Rust commits only `_mi_align_up(block_start + block_size, MI_PAGE_MIN_COMMIT_SIZE)` capped at `page_noguard_size`, preserves C-equivalent `memid.initially_committed`, `memid.initially_zero`, and `page.slice_committed` state, and lets `mi_page_extend_free` grow commits on demand; C-vs-Rust parity checks committed stats, commit-call counts, and allocation success over page extension.

## Heap, Theap, and Subprocess Behavior

- [x] Resolve custom subprocess main-heap behavior. Validation: after `mi_subproc_new` and `mi_subproc_add_current_thread`, `mi_heap_main` and `_mi_subproc_heap_main` behavior is useful and matches an authoritative C baseline or is documented as an intentional compatibility decision; replace or update `scripts/check-subproc-heap-main-baseline.py`, which currently validates that both vendored C and Rust abort on this path.

## Parity Evidence

- [x] Add focused regression coverage for every open allocator behavior item above. Validation: guarded-generic allocation and page commit-on-demand cases fail on the current Rust code, pass after the corresponding fix, and are included in the allocator-focused signoff with `cargo test`, `cargo test --release`, `cargo test --features secure`, `cargo test --features guarded`, `scripts/check-behavioral-parity.py`, `scripts/check-secure-mode-parity.py`, `scripts/check-guarded-mode-parity.py`, and the subprocess baseline or replacement script.

## Deferred Public ABI and Export Work

These are already documented in `PARITY.md`, but are not tracked as open items
in this allocator-focused TODO because they are public symbol or static-baseline
linkability issues rather than allocator-core behavior:

- `mi_stats_merge` and `mi_collect_reduce` are declared by the public headers, but the vendored C static baselines used by the repository's link checks do not provide linkable definitions.
- `mi_theap_set_default` is exported and smoke-tested in Rust, but the vendored C static baselines used by the repository's link checks do not provide a linkable public definition for direct comparison.
- `mi_uzalloc_small` is exported and smoke-tested in Rust, but the vendored C static baselines used by the repository's link checks do not provide a linkable public definition for direct comparison.

## Documented Items Not Added

- The documented C++ `std::bad_alloc` divergence is an accepted C ABI exception-path decision, not an allocator-core behavior bug.
- Unsupported matrix rows such as Windows, macOS, WASI, Emscripten, non-x86_64 targets, non-default debug levels, Valgrind tracking, sanitizer builds, and ETW/event tracking remain outside the current supported allocator parity matrix until `PARITY.md` adds them with evidence.
