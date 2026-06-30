# Public API Inventory

This inventory separates stable-intent surfaces from compatibility surfaces and
implementation internals.

## Public API Tiers

- Tier S: safe Rust API, Rust SemVer surface.
- Tier U: unsafe Rust convenience API, Rust SemVer surface, outside Tier S and
  not C-shaped raw API.
- Tier R: raw Rust facade, unsafe and unstable unless explicitly versioned.
- Tier C: exported C ABI and raw layout compatibility surface.
- Tier I: internal implementation surface, not public API.

## Crate Root

- `MiMalloc`: Tier S Rust `GlobalAlloc` facade.
- `Allocation`: Tier S owned byte allocation wrapper backed by `MiMalloc`.
- `InitializedAllocation`: Tier S initialized owned byte allocation wrapper.
- `Heap`, `HeapRef`, `MainHeap`, `HeapAllocation`, `InitializedHeapAllocation`:
  Tier S heap ownership and heap-tied allocation wrappers.
- `GlobalOptions`, `OptionKey`: Tier S supported allocator option controls.
- `StatsSnapshot`, `StatCount`, `StatTotal`: Tier S non-printing statistics
  snapshot wrappers.
- `ProcessInfo`: Tier S typed process allocator information.
- `AllocError`, `LayoutError`, `ResizeError`, `ResizeErrorKind`,
  `TryAllocError`, `HeapError`, `OptionError`, `StatsError`:
  Tier S Rust-native error/status types for the safe API.
- `raw`: Tier R explicit C-shaped compatibility facade.
- `unsafe_callbacks`: Tier U unsafe Rust callback guard namespace.

Implementation modules such as allocation, page, heap, arena, OS, primitive,
thread-local, and strict-type internals are crate-private and must not be
re-exported from the crate root.

## Safe Rust API

`MiMalloc` implements `core::alloc::GlobalAlloc`. `Allocation` owns one byte
allocation and releases it on drop, translating C-style null allocation failure
into Rust `Result` values. `InitializedAllocation` exposes safe `&[u8]` and
`&mut [u8]` access only after construction paths that guarantee initialized
bytes or an explicit unsafe initialization assertion.

`Heap`, `MainHeap`, `HeapRef`, `HeapAllocation`, and
`InitializedHeapAllocation` provide heap-scoped allocation without exposing raw
heap pointers through safe APIs. Heap-tied allocations borrow the heap so safe
code cannot delete an owned heap while live heap allocations still exist.

`GlobalOptions`, `StatsSnapshot`, and `ProcessInfo` cover supported runtime
controls and observations without requiring callers to pass raw out-pointers.
`GlobalOptions` mutates mimalloc's process-global option table; callers that
temporarily change options are expected to restore the previous values.
Unsupported or unavailable cases return typed Rust errors.

Borrowed callback guards are Tier U, not Tier S. Users who can uphold
process-global callback synchronization, lifetime, reentrancy, and allocator
entry obligations can use `unsafe_callbacks::ErrorCallbackGuard` and
`unsafe_callbacks::DeferredFreeCallbackGuard`.

Adding a safe wrapper requires documenting how it enforces ownership, lifetime,
aliasing, nullability, alignment, threading, and unwind requirements without
caller discipline.

Safe API design, naming, and trait rules are documented in `docs/safe-api.md`.

## Safe API Trait Inventory

- `MiMalloc`: implements `core::alloc::GlobalAlloc`; derives `Copy`, `Clone`,
  `Debug`, and `Default`; has auto `Send`/`Sync`; no manual unsafe `Send`/`Sync`
  impls.
- `Allocation`: implements `Drop` and `Debug`; has manual `Send`/`Sync` impls
  documented in `docs/unsafe-audit.md`; intentionally does not implement
  `Copy` or `Clone`.
- `InitializedAllocation`: implements `Drop` through `Allocation` ownership and
  `Debug`; intentionally does not expose uninitialized bytes.
- `Heap`: implements `Drop` and `Debug`; owns and deletes a mimalloc heap;
  intentionally not `Send` or `Sync`.
- `HeapRef`: `Copy` borrowed heap handle used to tie heap allocations to a
  heap lifetime; intentionally not `Send` or `Sync`.
- `MainHeap`: `Copy` handle for the process main heap; it never deletes the
  main heap; intentionally not `Send` or `Sync`.
- `HeapAllocation`: implements `Drop` and `Debug`; frees through the heap
  allocation family and carries the heap lifetime; intentionally not `Send` or
  `Sync`.
- `InitializedHeapAllocation`: initialized heap-tied byte allocation wrapper;
  intentionally not `Send` or `Sync`.
- `GlobalOptions`: namespace type for supported process-global safe allocator
  options.
- `OptionKey`: non-exhaustive supported-option key set; invalid raw keys are
  rejected.
- `StatsSnapshot`: non-printing copy of allocator statistics.
- `StatCount`, `StatTotal`: `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`,
  `PartialOrd`, and `Ord` typed integer wrappers.
- `ProcessInfo`: typed process allocator information. Size totals are `None`
  when the raw process-info API reports zero; `page_faults()` returns a direct
  count because zero is a valid observed count.
- `AllocError`: `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Display`, and
  `std::error::Error` when `std` is enabled.
- `LayoutError`: `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Display`, and
  `std::error::Error` when `std` is enabled.
- `ResizeError`, `ResizeErrorKind`, `HeapError`, `OptionError`, and
  `StatsError`: typed failure states for safe allocation, heap, option, and
  stats APIs; implement `Display` and `std::error::Error` when `std` is enabled
  where owned allocation preservation permits.
- `TryAllocError`: `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Display`, and
  `std::error::Error` when `std` is enabled.

## Tier U Unsafe Rust Callbacks

Tier U is public unsafe Rust API with Rust SemVer review, but it is not Tier S
safe facade API and is not Tier R/C-shaped raw API.

- `unsafe_callbacks::ErrorCallbackGuard`: Tier U borrowed `FnMut` RAII guard
  for the process-global error callback slot.
- `unsafe_callbacks::DeferredFreeCallbackGuard`: Tier U borrowed `FnMut` RAII
  guard for the process-global deferred-free callback slot.

These guards restore previous callback registrations on drop and catch panics
before callback ABI return in `std` and test builds, but their constructors are
unsafe because the callback slots are process-global and can be entered from
allocator activity outside the registering scope.
Nested guards must be dropped in LIFO order. Concurrent registration,
restoration, or callback entry for the borrowed closure remains the caller's
obligation.

## Raw Rust Facade

`mechanical_mimalloc::raw` exposes C-shaped functions, constants, callbacks,
and `repr(C)` layouts for tests, benchmarks, and low-level integration.

The raw facade is a compatibility surface, not a safe abstraction. Most calls
are unsafe because they inherit mimalloc's C preconditions.
The raw Rust module path surface is unstable until explicitly versioned.

By default, `raw` exposes the root C-header mirror plus `raw::abi`,
`raw::mimalloc`, and `raw::headers`. Translated implementation modules are
crate-private and must not be used as application integration paths.

The source/documentation inventory is checked by:

```sh
scripts/check-api-inventory.py
scripts/check-raw-facade.sh
```

Current raw users and raw module classifications are tracked in
`docs/raw-facade-migration.md`.

### Raw ABI Type Inventory

`src/ffi/abi.rs` re-exports these raw type names through `raw`, `raw::abi`, and
`raw::mimalloc`:

| Type | Public shape | Opacity decision |
| --- | --- | --- |
| `mi_arena_id_t` | `*mut c_void` token | Opaque arena handle. C callers and raw Rust callers pass it back to arena APIs; no Rust layout is exposed. |
| `mi_block_visit_fun` | C callback type alias | Signature is public ABI. The pointed-to heap and area data remain allocator-owned for the callback duration. |
| `mi_commit_fun_t` | C callback type alias | Signature is public ABI for managed-memory hooks. User data is opaque. |
| `mi_deferred_free_fun` | C callback type alias | Signature is public ABI for deferred-free registration. User data is opaque. |
| `mi_error_fun` | C callback type alias | Signature is public ABI for error callback registration. User data is opaque. |
| `mi_heap_area_t` | `repr(C)` public field layout | Public because visitor callbacks receive area metadata by pointer and C parity tests verify the layout. |
| `mi_heap_t` | raw heap handle alias | Kept as the translated C layout alias for current raw-layout compatibility tests and C-shaped heap APIs. Safe Rust code uses `Heap`, `HeapRef`, and `MainHeap` instead. |
| `mi_heap_visit_fun` | C callback type alias | Signature is public ABI. Heap pointer is allocator-owned and callback-scoped. |
| `mi_output_fun` | C callback type alias | Signature is public ABI for printing and registered output callbacks. User data is opaque. |
| `mi_subproc_id_t` | `repr(C)` opaque identifier | Public by-value C token with a private representation field. Callers can pass it back to subproc APIs but cannot inspect or construct it from fields. |
| `mi_theap_t` | raw thread-heap handle alias | Kept as the translated C layout alias for current raw-layout compatibility tests and C-shaped thread-heap APIs. Safe Rust code does not expose thread-heap internals. |

`mi_arena_t`, `mi_subproc_t`, `mi_page_t`, `mi_block_t`, and other translated
implementation layouts are not re-exported by the default `src/ffi/abi.rs` public
header mirror. They are reachable only through private implementation code,
except where an exposed callback or handle uses an opaque pointer token.

### Raw `repr(C)` Field Inventory

`src/types/constants.rs` remains the translated layout substrate. Default raw ABI
exposure is intentionally narrower than the full file:

| Struct | Default raw ABI exposure | Field visibility decision |
| --- | --- | --- |
| `mi_process_info_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain translated layout for internal/stat paths. |
| `mi_random_ctx_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain translated thread-heap layout parity. |
| `mi_page_queue_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain translated page queue layout parity. |
| `mi_block_s` / `mi_block_t` | Not re-exported by `src/ffi/abi.rs` today | Public field retains translated free-list word layout. |
| `mi_padding_s` / `mi_padding_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain debug/secure padding layout. |
| `mi_heap_area_t` | Re-exported by `src/ffi/abi.rs` | Public fields are required for heap/block visitor metadata compatibility. |
| `mi_arena_pages_s` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain translated arena metadata layout. |
| `mi_subproc_s` / `mi_subproc_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields are private implementation layout. |
| `mi_tld_s` / `mi_tld_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields are private implementation layout. |
| `mi_arena_s` / `mi_arena_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields are implementation layout; the default API uses opaque `mi_arena_id_t`. |
| `mi_heap_s` / `mi_heap_t` | `mi_heap_t` is re-exported by `src/ffi/abi.rs` | Layout alias is retained for raw heap handle compatibility; safe heap APIs stay opaque. |
| `mi_page_s` / `mi_page_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain translated page layout for internal layout tests. |
| `mi_theap_s` / `mi_theap_t` | `mi_theap_t` is re-exported by `src/ffi/abi.rs` | Layout alias is retained for raw thread-heap compatibility and offset tests. |
| `mi_tls_slot_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain TLS mirror layout. |
| `mi_thread_locals_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain TLS mirror layout. |
| `mi_option_desc_t` | Not re-exported by `src/ffi/abi.rs` today | Public fields retain translated option table layout. |
| `mi_subproc_id_t` | Re-exported by `src/ffi/abi.rs` | `repr(C)` size is public, but the field is private so the identifier is opaque to downstream Rust. |

### Macro Production Contract

Exported macros are not Tier S safe Rust API. The production decision is:

- Keep `tier-r-typed-allocation` macros as Tier R C-header compatibility
  macros for this release behind the `c-header-macros` feature. They remain
  unsafe raw-facade conveniences, not recommended safe Rust allocation APIs.
- Keep `tier-r-header-override` macros as Tier R C/C++ header compatibility
  macros for this release behind the `c-header-macros` feature.
- Treat `internal-compatibility` macros as temporary compatibility exports.
  Before a stable release, each macro in that bucket must be made
  crate-private, removed, or explicitly retained with downstream expansion,
  translated parity, or hot-path evidence. New internal macros must not use
  `#[macro_export]` unless they are first classified here.

### Typed Allocation Macros

Macro tier: `tier-r-typed-allocation`

The typed allocation macros in `src/ffi/abi.rs` are Tier R public compatibility
surface when the `c-header-macros` feature is enabled. They mirror mimalloc
C-header convenience spellings and expand through public `raw::mi_*` paths
without internal `unsafe` blocks.

- `mi_malloc_tp!`
- `mi_zalloc_tp!`
- `mi_calloc_tp!`
- `mi_mallocn_tp!`
- `mi_reallocn_tp!`
- `mi_heap_malloc_tp!`
- `mi_heap_zalloc_tp!`
- `mi_heap_calloc_tp!`
- `mi_heap_mallocn_tp!`
- `mi_heap_reallocn_tp!`
- `mi_heap_recalloc_tp!`

### Header Override Macros

Macro tier: `tier-r-header-override`

The exported macros in `src/ffi/headers.rs` are Tier R public compatibility
surface when the `c-header-macros` feature is enabled. They model C header
override and C++ operator shim spellings while expanding through public
`raw::mi_*` paths so downstream crates do not observe implementation modules.

Allocation and string macros:

- `malloc!`
- `realloc!`
- `strdup!`
- `_strdup!`
- `realpath!`
- `reallocf!`
- `malloc_usable_size!`

POSIX and platform allocation macros:

- `reallocarray!`
- `posix_memalign!`
- `cfree!`
- `valloc!`
- `memalign!`
- `_wcsdup!`
- `_dupenv_s!`

Aligned and Windows-style allocation macros:

- `_expand!`
- `_recalloc!`
- `_aligned_recalloc!`
- `_aligned_malloc!`
- `_aligned_free!`
- `_aligned_offset_realloc!`

C++ operator shim macros:

- `operatornew!`
- `operatornew_array!`
- `operatornew_nothrow!`
- `operatornew_array_nothrow!`
- `operatornew_aligned!`
- `operatornew_array_aligned!`
- `operatornew_aligned_nothrow!`
- `operatornew_array_aligned_nothrow!`
- `operatordelete!`
- `operatordelete_array!`
- `operatordelete_nothrow!`
- `operatordelete_array_nothrow!`
- `operatordelete_sized!`
- `operatordelete_array_sized!`
- `operatordelete_aligned!`
- `operatordelete_array_aligned!`
- `operatordelete_sized_aligned!`
- `operatordelete_array_sized_aligned!`
- `operatordelete_aligned_nothrow!`
- `operatordelete_array_aligned_nothrow!`

### Internal Compatibility Macro Exports

Macro tier: `internal-compatibility`

These `#[macro_export]` macros remain crate-root public because translated code
uses upstream-style macro expansion and some downstream macro expansion tests
exercise the same crate-root behavior. They are compatibility exports, not a
recommended integration surface; new internal macros should not be exported
unless downstream expansion or translated C-header parity requires it.

The forwarding macros `MI_FORWARD1!`, `MI_FORWARD3!`, and `MI_FORWARD02!`
were unused and have been removed from the crate-root macro surface. The raw
Darwin interpose helper `MI_INTERPOSE_FUN!` is now file-local in
`src/raw/macros.rs`, and the unused `MI_INTERPOSE_MI!` wrapper was removed.
The stats field/declaration helpers `MI_STAT_COUNT!`, `MI_STAT_COUNTER!`,
`MI_STAT_FIELDS!`, and `mi_stats_t_decl!` were removed from the Rust macro
surface; internal stats paths now initialize `mi_stats_t` through a private
function and downstream C parity remains covered by the vendored C headers.

Forwarding and interposition macros:

- None remain exported.

Initialization, assertion, and utility macros:

- `MI_INIT4!`
- `MI_INIT5!`
- `MI_INIT6!`
- `MI_INIT8!`
- `MI_INIT16!`
- `MI_INIT32!`
- `MI_INIT64!`
- `MI_INIT74!`
- `MI_INIT128!`
- `MI_INIT256!`
- `MI_UNUSED!`
- `MI_UNUSED_RELEASE!`
- `_mi_memzero_var!`
- `mi_assert!`
- `mi_assert_internal!`
- `mi_assert_expensive!`
- `mi_debug_assert!`
- `mi_debug_assert_eq!`
- `mi_trace_message!`

Statistics and once/guard macros:

- `mi_theap_stat_counter_increase!`
- `mi_theap_stat_decrease!`
- `mi_theap_stat_adjust_decrease!`
- `mi_heap_stat_counter_increase!`
- `mi_heap_stat_decrease!`
- `mi_heap_stat_adjust_decrease!`
- `mi_subproc_stat_counter_increase!`
- `mi_subproc_stat_decrease!`
- `mi_subproc_stat_adjust_decrease!`
- `mi_os_stat_counter_increase!`
- `mi_os_stat_decrease!`
- `mi_atomic_guard!`
- `mi_atomic_do_once!`

## docs.rs Behavior

`Cargo.toml` builds docs.rs with the `std` feature so safe error traits and
examples render normally. There are no docs.rs-only public APIs; `docsrs` is
reserved for documentation configuration. Benchmarks are separate Cargo bench
targets and are not presented as normal library APIs in rustdoc.

## C ABI Symbols

The Tier C surface is exported by the opt-in `cdylib` artifact built with the
explicit interposition feature set:

```sh
cargo rustc --features interpose --lib --release -- --crate-type cdylib
```

The authoritative local checks are:

```sh
scripts/check-abi-symbols.sh
scripts/check-layout.sh
scripts/check-feature-abi.sh
```

Extra exported symbols outside the public C headers must be classified in
`symbols/public-symbol-classification.tsv`; otherwise the symbol check fails.
Rust-only `mi_*` helpers classified there are not default dynamic C ABI exports;
they are hidden from `c-abi` cdylibs unless a later release explicitly promotes
one as documented extension ABI.
`scripts/check-feature-abi.sh` repeats the ABI/layout and export checks for the
layout- or symbol-sensitive `secure`, `guarded`, `track-valgrind`, and
`track-asan` feature sets.

## Hidden Internals

Tier I internal C-shaped helper functions may remain `extern "C"` when translated code
depends on that shape, but they must not become dynamic ABI exports unless the
symbol is deliberately classified. `tests/release_fast_path_audit.rs` guards
known internal helpers against regaining `#[unsafe(no_mangle)]`.

New crate-root public items, new raw modules, or new export attributes outside
approved FFI boundary files are release-blocking until this inventory and the
corresponding source-shape checks are updated.

## SemVer Classification

- Crate root safe API: Rust SemVer surface.
- `raw`: unstable low-level Rust path surface until explicitly versioned.
- C ABI symbols and layouts: C compatibility surface.
- `types`: private design surface, not public SemVer.
- Benchmarks, scripts, and docs: release-process surface, not allocator API.

Detailed compatibility domains and release classification rules are documented
in `docs/compatibility.md`.
