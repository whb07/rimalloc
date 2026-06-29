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

C integer domains that can receive values from C callers must not rely on Rust
enum validity unless the domain is closed. Public raw option values use the
transparent integer newtype `mi_option_t`; known option constants are associated
constants, and unknown values can be represented with `mi_option_t::from_raw`
without creating Rust enum UB. Closed internal translated enums may remain Rust
enums when they are not part of the default public raw facade.

## Raw Rust Facade

`mechanical_mimalloc::raw` is the Rust path for C-shaped constants, callbacks,
layouts, and functions. It is intentionally separate from the crate root:

- crate root: safe `MiMalloc` facade and `raw`
- default `raw` root: public `mi_*` C-header re-exports
- `raw::abi`: `repr(C)` layouts, C-compatible aliases, constants, callback type
  aliases, and public C-header function declarations
- `raw::mimalloc`: direct unsafe C-shaped public functions and constants for
  low-level Rust callers that intentionally want the mimalloc C shape
- `raw::headers`: translated public-header macros and compatibility items that
  do not belong in the ABI declaration owner
- `unstable-raw-internals`: gated translated implementation modules for
  repository audits and migration checks only

Most raw functions are unsafe because pointer validity, allocation origin,
alignment, lifetime, nullability, and threading are caller obligations.
The `unstable-raw-internals` module paths are not public API and must not be
used by downstream users as normal library paths.

## Raw Pointer Nullability

The raw C boundary uses raw pointers in signatures. It does not use `NonNull`
in raw C signatures because the C header shape must represent nullable
pointers, allocation-failure null returns, and pointer domains whose
nullability is inherited from upstream mimalloc.

Nullability follows the C API contract and parity tests:

- allocation functions return null on allocation failure, overflow, or invalid
  requests where upstream returns null;
- `mi_free` and matching deallocation APIs accept null as a no-op where the C
  API does;
- `mi_realloc` and matching realloc-family APIs treat a null input pointer as
  an allocation request only for functions where the C API does;
- out-pointers require valid writable storage unless the function docs or C
  behavior explicitly allow null;
- heap, thread-heap, arena, page, and other metadata pointers must be valid and
  non-null unless the specific raw function documents a nullable case.

Unsafe Rust convenience wrappers and allocator-internal helpers may use
`NonNull` only after construction proves the pointer is not null, or after a
checked conversion such as `NonNull::new`. Those wrappers must convert back to
raw `*mut` or `*const` pointers at the raw C boundary.

Safe Rust APIs must not expose C null/error conventions directly. Safe wrappers
translate nullable C results into `Result`, `Option`, safe no-op behavior, or a
non-null handle such as `NonNull` when null has already been checked.

## Module Taxonomy

Current ownership is the final layout for this migration phase:

| Layer | Current owner | Responsibility |
| --- | --- | --- |
| Safe Rust facade | `src/lib.rs`, `src/safe/mod.rs` | `MiMalloc`, `Allocation`, and Rust error/status types. |
| C header declarations and layouts | `src/ffi/abi.rs` exposed through `raw::abi` and `raw::mimalloc` | Public `mi_*` declarations, `repr(C)` layouts, C-compatible aliases, constants, and callback type aliases mirrored from public headers. |
| Raw C-shaped Rust facade | `src/raw/mod.rs` | Default root `mi_*` re-exports plus `raw::abi`, `raw::mimalloc`, and `raw::headers`; translated internals are gated by `unstable-raw-internals`. |
| Exported C ABI implementations | existing implementation files approved by `scripts/check-ffi-boundary.py` | `#[unsafe(no_mangle)]`, `#[unsafe(export_name = ...)]`, naked ABI shims, and public assembly aliases. |
| Strict internal types | `src/types.rs` and `src/types/` | Private invariant-carrying wrappers and witness types used inside allocator internals; never part of the raw C facade or C ABI declaration owner. |
| Internal compatibility aliases | `src/ffi/mimalloc.rs` and selected translated helpers | C-shaped helper paths used by the mechanical translation but not a public Rust API. |
| Allocator internals | private modules such as `alloc`, `free`, `heap`, `page`, `arena`, `os`, and `threadlocal` | Implementation logic and typed invariant witnesses. |

Module flow:

```text
safe crate root
  -> src/safe/mod.rs
  -> raw C-shaped calls

mechanical_mimalloc::raw
  -> src/raw/mod.rs
  -> src/ffi/abi.rs / src/ffi/headers.rs

C ABI exports
  -> classified src/*.rs files checked by scripts/check-ffi-boundary.py
  -> private allocator internals

unstable raw internals
  -> feature-gated src/raw/mod.rs modules
  -> private translated modules
```

`raw.rs` must remain a structured facade, not an unbounded re-export bucket.
`scripts/check-api-inventory.py` enforces the default raw module set and the
feature-gated `unstable-raw-internals` module set.

## Current File Layout

This project keeps the ABI declaration owner flat for now instead of creating a
new `src/raw/` or `src/ffi/` tree. A structural move can happen later only with
the same parity evidence and inventory checks.

| New work | File owner |
| --- | --- |
| New public C ABI declarations, `repr(C)` layouts, C-compatible aliases, public constants, or callback type aliases | `src/ffi/abi.rs`, exposed through `raw::abi` and `raw::mimalloc` |
| New public-header macro translations or header-level compatibility helpers | `src/ffi/headers.rs`, exposed through `raw::headers` when they are part of the raw header mirror |
| New exported C ABI implementations or symbol aliases | the matching classified implementation file checked by `scripts/check-ffi-boundary.py`; update that script if a new export file is introduced |
| New malloc-family or C++ override symbols | `src/alloc_override.rs` or a classified platform override file |
| New runtime, init, assembly, or platform ABI shims | `src/init.rs`, `src/prim.rs`, `src/prim/unix.rs`, or another file classified by `scripts/check-ffi-boundary.py` |
| New internal C-shaped compatibility alias needed by translated code | `src/ffi/mimalloc.rs` or a private translated module, not the default `raw` facade |
| New allocator implementation logic | private allocator modules such as `alloc`, `free`, `heap`, `page`, `arena`, `os`, or `threadlocal`; do not re-export it from default `raw` |

There should be no ambiguity about where to add a new C ABI declaration or an
exported function: declarations live in `src/ffi/abi.rs`, and exported
implementations live only in files classified by `scripts/check-ffi-boundary.py`.

## Export Boundary Classification

Exported C ABI functions are boundary wrappers even when they remain in the
translated implementation file for parity with upstream C. `scripts/check-ffi-boundary.py`
owns the allowlist of files that may contain `#[unsafe(no_mangle)]`,
`#[unsafe(export_name = ...)]`, naked ABI shims, `global_asm!`, or `naked_asm!`
public aliases. Run `scripts/check-ffi-boundary.py --list` to list every
classified export site with its file, line, and boundary class.

The current explicit boundary classes are:

- `c-abi`: public `mi_*` and related C API wrappers that preserve C null,
  errno, and allocation semantics.
- `malloc-cxx-override`: malloc-family and C++ operator override symbols whose
  names are part of the dynamic ABI surface.
- `runtime-boundary` and `panic-runtime-boundary`: process/thread lifecycle,
  panic, and platform runtime hooks.
- `c-abi-internal-classified`: translated internal `_mi_*` helpers that are
  still exported for C parity or static archive compatibility and are tracked
  separately from stable public API.

Some naked shims and assembly aliases intentionally remain near allocation,
free, runtime, or override code instead of moving to a separate file. Those
sites are classified because their exact symbol spelling, aliasing, and codegen
shape are part of the C/malloc override contract or release fast path audit.
Moving them requires ABI symbol checks, release fast-path checks, and benchmark
evidence.

Internal allocator code must not call `raw::mi_*` or `abi::mi_*` public entry
points to reach implementation logic. It should call crate-private helpers so
Rust-side invariants, typed wrappers, and non-ABI call paths remain visible.
The only current allowlisted source re-entry is the public C header macro group
in `src/ffi/abi.rs`; those macros intentionally expand through root `raw::mi_*` so
downstream macro users see C-header behavior. `scripts/check-ffi-boundary.py`
fails if any other source file re-enters the public raw/ABI `mi_*` facade.

## Raw Macro Unsafe Boundary

Public raw macros are compatibility conveniences over the raw C-shaped facade,
not safe Rust wrappers.

- Typed allocation macros in `src/ffi/abi.rs` (`mi_malloc_tp!`, `mi_zalloc_tp!`,
  `mi_calloc_tp!`, `mi_mallocn_tp!`, `mi_reallocn_tp!`, and the heap typed
  variants) expand directly to `raw::mi_*` calls and pointer casts. They do not
  contain internal `unsafe` blocks, so callers must use them inside an unsafe
  boundary. The compile-fail doctests in `src/lib.rs` cover each typed macro
  form outside unsafe.
- Header override macros in `src/ffi/headers.rs` (`malloc!`, `realloc!`, C++
  operator shims, aligned allocation shims, and related header spellings)
  likewise expand through public `raw::mi_*` calls without internal `unsafe`
  blocks. They require caller-side unsafe for the same pointer ownership,
  layout, lifetime, and threading obligations as the raw functions they call.
- There are no intentionally safe public allocation macros today. Any future
  safe macro must expand only to safe APIs such as `Allocation` or `Heap`
  wrappers and must be documented separately from the raw macro boundary.

## Naming Rules

The dynamic C ABI preserves upstream mimalloc symbol spelling. Public C symbols,
C-header declarations, raw facade exports, and direct compatibility shims keep
their `mi_*` or `_mi_*` names when that spelling is part of the C contract.

New allocator-internal Rust helpers should use Rust names or typed methods when
the helper is not itself a C compatibility shim. Prefer names that describe the
Rust invariant being carried, such as `ValidatedFree`, `PointerLookup`,
`LayoutRequest`, `calloc_request_or_report`, or methods on those types. Do not
add new internal `mi_*` entry points just to mirror C spelling; reserve that
shape for translated compatibility shims whose ABI, debug, static-archive, or
upstream-parity role is explicit.

`docs/safe-api.md` owns Rust-facing safe API naming conventions. This document
owns the C ABI/raw boundary naming split.

Allocation and free entry points convert raw C inputs into crate-private
witness types before entering allocator-core paths when validation is possible:

- size/count/alignment inputs use `LayoutRequest`, `CallocRequest`,
  `ReallocRequest`, `RequestedSize`, `MaybeAlignment`, and
  `MaybeHugeAlignment`;
- old realloc pointers use `PointerLookup` and construct `OldAllocation` only
  after pointer/page/block validation;
- free paths use `PointerLookup`, `BlockForFree`, and `ValidatedFree` before
  routing local or foreign frees;
- generic allocation flags use `MallocGenericFlags` instead of ad hoc bit
  packing.

C boundary wrappers still return C-shaped results: null for allocation failure
or invalid/overflow requests where mimalloc returns null, errno-compatible
behavior where the public C API requires it, and abort/error reporting where the
upstream C contract does so.

## Callback ABI

Callback types mirror upstream mimalloc C callback signatures. Public callback
function pointers are represented as `Option<mi_*_fun>` where the C API accepts
a null callback. Callbacks must not unwind across the C ABI boundary. Callers
are responsible for ensuring callback function pointers and callback arguments
remain valid for the documented registration lifetime or direct-call duration.

Default public callback families:

| Family | Type | Public entry points | Nullability | Lifetime | Threading | Reentrancy |
| --- | --- | --- | --- | --- | --- | --- |
| Output callback | `mi_output_fun` | `mi_register_output`, `mi_options_print_out`, `mi_process_info_print_out`, `mi_stats_print_out`, `mi_thread_stats_print_out`, `mi_heap_stats_print_out`, `mi_subproc_stats_print_out`, `mi_subproc_heap_stats_print_out`, deprecated `mi_stats_print` callback pointer form | Callback may be null to use/restore defaults or clear registration. `msg` is non-null during callback invocation. `arg` may be null and is passed through opaquely. | Registered callback and `arg` must remain valid until replaced or cleared; direct print callbacks must remain valid for the call duration. | May be invoked by any thread that emits diagnostics, options, process info, or stats output. | Allocating from the callback is C-parity-tested, but callback code must tolerate allocator reentry and must not rely on recursive diagnostics being serialized beyond the C contract. |
| Error callback | `mi_error_fun` | `mi_register_error` | Callback may be null to clear registration. `arg` may be null and is passed through opaquely. | Callback and `arg` must remain valid until replaced or cleared. | May be invoked by any thread reporting allocator errors. | Allocating from the callback is C-parity-tested, but callback code must tolerate allocator reentry. |
| Deferred free callback | `mi_deferred_free_fun` | `mi_register_deferred_free` | Callback may be null to clear registration. `arg` may be null and is passed through opaquely. | Callback and `arg` must remain valid until replaced or cleared. | May be invoked from allocator activity on a thread using the registered hook. | Allocating from the callback is C-parity-tested, but callback code must tolerate allocator reentry. |
| Heap visitor callback | `mi_heap_visit_fun` | `mi_subproc_visit_heaps` | Callback may be null. `heap` is non-null for normal visitor invocation. `arg` may be null. | Callback and `arg` must remain valid for the traversal call; the heap pointer is allocator-owned and valid only during the callback. | Invocation follows the calling traversal and allocator state. | Allocating from the callback is C-parity-tested for heap traversal, but callback code must tolerate allocator reentry. |
| Block visitor callback | `mi_block_visit_fun` | `mi_heap_visit_blocks`, `mi_heap_visit_abandoned_blocks`, `mi_theap_visit_blocks` | Callback may be null. `heap` and `area` are non-null for normal visitor invocation. `block` is non-null when block visitation is requested and may be null for area-only visitation. `arg` may be null. | Callback and `arg` must remain valid for the traversal call; allocator-owned pointers are valid only during the callback. | Invocation follows the calling traversal and allocator state. | Allocating from the callback follows the C contract; visitors must tolerate allocator reentry and traversal invalidation rules. |
| Managed-memory commit callback | `mi_commit_fun_t` | `mi_manage_memory` | Callback may be null. `start` is non-null for normal commit/decommit ranges. `is_zero` may be null when no zero-state result is requested. `user_arg` may be null. | Callback and `user_arg` must remain valid for the managed arena lifetime that stores them. | May be invoked from threads using the managed arena. | Allocating from the callback is C-parity-tested, but callback code must tolerate allocator reentry and must not invalidate the managed range. |

Translated internal callback helpers such as `mi_claim_fun_t`,
`mi_forall_set_fun_t`, `theap_page_visitor_fun`, `mi_theap_area_visit_fun`, and
bitmap visitor helpers are implementation-internal or exposed only through the
`unstable-raw-internals` audit surface. They are not part of the default public
raw facade and must not be stabilized without adding them to this inventory and
documenting the same nullability, lifetime, threading, reentrancy, and unwind
contract.

Raw callback registration remains unsafe. `unsafe_callbacks::ErrorCallbackGuard`
and `unsafe_callbacks::DeferredFreeCallbackGuard` wrappers are available for
callback families where the crate can tie the callback context to a Rust borrow,
restore the previous registration on drop, and catch panic or unwind before
returning across the callback C ABI boundary. They are unsafe-adjacent Rust APIs,
not Tier S safe facade APIs. Output callbacks remain raw until output recursion
and diagnostic allocation semantics are designed.
`unsafe_callbacks::ErrorCallbackGuard` and
`unsafe_callbacks::DeferredFreeCallbackGuard` are the only borrowed guard
wrappers currently provided.
Those wrappers catch panic or unwind before returning across the
callback C ABI boundary in `std` and test builds.

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
When `track-valgrind` and `track-asan` are both enabled, Valgrind mode takes
precedence.

Before claiming ABI compatibility for a feature set, run the relevant feature
sweep and ABI/layout checks for that feature set or document why the feature
does not affect public layout or symbols.
