# Init Preloading Regression Report

Date: 2026-06-26

## Summary

`cargo test --all-features` currently fails in `tests/original_main_override_static.rs` because normal allocator API initialization can leave `OS_PRELOADING` set to `true`.

This appears to be a real allocator lifecycle regression, not a broken test. While `OS_PRELOADING` remains true:

- lazy arena reservation is skipped, so allocations fall back to `MI_MEM_OS`;
- heap walking sees no live arena pages for those allocations;
- arena purge scheduling/collection is skipped, so forced collection does not increase `purge_calls`.

The long-term fix should make the public process/thread initialization path reach the same "post attach" state as the constructor path. Do not fix this by making the tests call `mi_process_attach()` directly; that hides the lifecycle bug, especially on macOS/arm64 where the generic `.init_array` attach hook is not registered.

## Failing Tests

Full suite command:

```sh
cargo test --all-features
```

Observed failure:

```text
Running tests/original_main_override_static.rs

internal_arena_purge_does_not_recommit ... FAILED
heap_walk_smoke ... FAILED

heap_walk_smoke:
assertion failed: state.areas > 0

internal_arena_purge_does_not_recommit:
forced collection should purge the freed internal arena page
```

Isolated commands also fail:

```sh
cargo test --all-features --test original_main_override_static heap_walk_smoke -- --exact --nocapture
cargo test --all-features --test original_main_override_static internal_arena_purge_does_not_recommit -- --exact --nocapture
```

## What The Tests Expect

`heap_walk_smoke`:

- creates a heap with `mi_heap_new`;
- allocates four blocks on that heap;
- calls `mi_heap_visit_blocks(heap, true, ...)`;
- expects at least one area and at least one block to be visited.

`internal_arena_purge_does_not_recommit`:

- sets `mi_option_purge_delay = 0`;
- sets `mi_option_purge_decommits = 1`;
- allocates and touches 8 MiB;
- frees it and forces collection;
- expects `purge_calls.total` to increase and `commit_calls.total` to remain unchanged.

Those expectations are valid when allocations are arena-backed.

## Runtime Evidence

I used a temporary probe outside the repo at `/private/tmp/mm_probe` that depends on this crate with the same feature set. It printed allocation memids, heap walk counts, and purge counters. The probe was not committed.

Without explicit process attach:

```sh
cargo +1.92.0 run --quiet
```

Key observations:

```text
options arena_reserve=1048576 disallow_arena_alloc=0 arena_max_object_size=2097152 disallow_os_alloc=0

alloc size=33554432 ... memkind=MI_MEM_OS ... arena_count=0
alloc size=2067152  ... memkind=MI_MEM_OS ... arena_count=0
alloc size=2097160  ... memkind=MI_MEM_OS ... arena_count=0
alloc size=24576    ... memkind=MI_MEM_OS ... arena_count=0

visit ok=true areas=0 blocks=0

purge alloc ... memkind=MI_MEM_OS
purge_calls before=0 after=0 commit_calls before=0 after=0 purged before=0 after=0 arena_purges before=0 after=0
```

With explicit process attach:

```sh
cargo +1.92.0 run --quiet -- attach
```

Key observations:

```text
alloc size=33554432 ... memkind=MI_MEM_ARENA ... arena_count=1
alloc size=2067152  ... memkind=MI_MEM_ARENA ... arena_count=1
alloc size=2097160  ... memkind=MI_MEM_ARENA ... arena_count=1
alloc size=24576    ... memkind=MI_MEM_ARENA ... arena_count=1

visit ok=true areas=4 blocks=1

purge alloc ... memkind=MI_MEM_ARENA
purge_calls before=0 after=1 commit_calls before=0 after=0 purged before=0 after=9502720
```

Explicit arena reservation also works:

```sh
cargo +1.92.0 run --quiet -- reserve 1073741824
cargo +1.92.0 run --quiet -- reserve 134217728
```

Both returned `reserve err=0`, increased `arena_count` to `1`, and subsequent allocations were `MI_MEM_ARENA`. This shows arena management itself is functional; the regression is the default initialization/preloading state.

## Static Evidence

Relevant current code:

- `OS_PRELOADING` starts as `true` in `src/init.rs`.
- `_mi_preloading()` reads `OS_PRELOADING`.
- `_mi_auto_process_init()` clears it with `OS_PRELOADING.store(false, Ordering::Relaxed)`.
- `mi_process_init()` currently calls only `mi_process_init_inner()`, not `_mi_auto_process_init()`.
- `mi_thread_init()` calls `mi_process_init()`.

Important paths:

```text
src/init.rs:
  mi_thread_init() -> mi_process_init()
  mi_process_init() -> mi_process_init_inner()
  _mi_auto_process_init() -> OS_PRELOADING.store(false, ...)
```

Arena allocation skips lazy reserve while preloading:

```text
src/arena.rs:
  if req_arena.is_some() || _mi_preloading() {
      return ptr::null_mut();
  }
```

Arena purge scheduling/collection also skips while preloading:

```text
src/arena.rs:
  if arena.memid.is_pinned || delay < 0 || _mi_preloading() { return; }
  if _mi_preloading() || delay <= 0 { return; }
```

Heap visiting walks arena page bitmaps:

```text
src/arena.rs:
  mi_bitmap_forall_set_typed((*arena_pages).pages, ...)
```

If allocations fell back to `MI_MEM_OS`, they were never inserted into that arena bitmap, so `mi_heap_visit_blocks` can return `true` while visiting zero areas.

## Git History

`heap_walk_smoke` was introduced first:

```text
239225727c6c16546d67dc695421c305125bd220
2026-06-07
Translate main-override-static test batch
```

`internal_arena_purge_does_not_recommit` was added later:

```text
f7517288f85115190670df85a3a9550f972a86eb
2026-06-14
Replace mi_commit_fun_t raw pointer with Option<mi_commit_fun_t>
```

The likely regressing commit is:

```text
1f9cdd6608c983ee3b99c393658691637dc3f189
2026-06-15
Honor preload state during process init
```

That commit:

- introduced `OS_PRELOADING: AtomicBool = AtomicBool::new(true)`;
- changed `_mi_preloading()` from hardcoded `false` to reading `OS_PRELOADING`;
- changed `mi_thread_init()` from calling `_mi_auto_process_init()` to calling `mi_process_init()`;
- changed `mi_process_init()` from calling `_mi_auto_process_init()` to calling only `mi_process_init_inner()`;
- left `OS_PRELOADING.store(false, ...)` only in `_mi_auto_process_init()`.

This static shape matches the runtime behavior exactly.

Later changes to `tests/original_main_override_static.rs` on 2026-06-18 are raw facade/import migrations and do not appear to change the semantics of the failing assertions.

## Cross-Platform Considerations

Linux/x64:

- `src/prim.rs` registers `_MI_PROCESS_ATTACH_CTOR` in `.init_array` for `target_family = "unix"` and `not(target_vendor = "apple")`.
- That constructor calls `mi_process_attach()`, which calls `_mi_auto_process_init()`, which clears `OS_PRELOADING`.
- However, relying only on the constructor is brittle in Rust test/static-link contexts, and public API initialization should still leave the allocator in a fully initialized state.

macOS/arm64:

- `src/prim.rs` explicitly excludes Apple from the generic `.init_array` attach hook.
- `src/prim/osx.rs` has a macOS malloc-zone constructor under `mi_osx_interpose`, but it registers/reorders malloc zones and does not call `mi_process_attach()`.
- Therefore macOS/arm64 must not depend on the generic constructor path to clear `OS_PRELOADING`; the first public allocator API initialization path must do it.

The fix must make public initialization attach-equivalent on both platforms.

## Recommended Long-Term Fix

Restore the invariant that public process initialization completes the post-preload transition.

Preferred minimal fix:

```rust
#[unsafe(no_mangle)]
/// # Safety
/// Public C ABI wrapper for process-global initialization.
pub unsafe extern "C" fn mi_process_init() {
    unsafe { _mi_auto_process_init() };
}
```

Keep `mi_thread_init()` calling `mi_process_init()`.

Rationale:

- This restores the pre-regression behavior from before `1f9cdd66`.
- `_mi_auto_process_init()` is already the attach-equivalent path.
- It clears `OS_PRELOADING`.
- It is idempotent around the core one-time initialization through `PROCESS_INIT`.
- It works when Linux/x64 constructors have already run, because the one-time inner init is guarded.
- It works on macOS/arm64 where the generic attach constructor is not registered.

If there is concern that `_mi_auto_process_init()` contains too much constructor-specific work, use a shared helper instead:

```text
mi_process_activate_after_preload()
  - OS_PRELOADING.store(false, Relaxed)
  - mi_process_init_inner()
  - mi_heap_main_init()
  - mi_tls_slots_init()
  - options post-init once
  - redirector / allocator init hook sequence
  - weak random reinit

_mi_auto_process_init() calls mi_process_activate_after_preload()
mi_process_init() calls mi_process_activate_after_preload()
```

Do not clear `OS_PRELOADING` in allocation hot paths or in arena allocation itself. The preload state is a process lifecycle state; allocator subsystems should continue to consult it but not own its transition.

## Tests To Run After The Fix

Targeted:

```sh
cargo test --all-features --test original_main_override_static heap_walk_smoke -- --exact --nocapture
cargo test --all-features --test original_main_override_static internal_arena_purge_does_not_recommit -- --exact --nocapture
```

Full suite:

```sh
cargo test --all-features
```

Cross-platform matrix:

```text
macOS arm64:
  cargo test --all-features --test original_main_override_static heap_walk_smoke -- --exact --nocapture
  cargo test --all-features --test original_main_override_static internal_arena_purge_does_not_recommit -- --exact --nocapture
  cargo test --all-features

Linux x64:
  cargo test --all-features --test original_main_override_static heap_walk_smoke -- --exact --nocapture
  cargo test --all-features --test original_main_override_static internal_arena_purge_does_not_recommit -- --exact --nocapture
  cargo test --all-features
```

Optional static regression coverage:

- Add a release/audit test that checks `mi_process_init()` delegates to `_mi_auto_process_init()` or otherwise clears `OS_PRELOADING` through the shared attach-equivalent helper.
- Keep `heap_walk_smoke` and `internal_arena_purge_does_not_recommit`; they are useful behavioral coverage.
- Improve `heap_walk_smoke` diagnostics by asserting the allocation pointers are non-null before visiting blocks.

## What Not To Do

- Do not fix the test by calling `raw::prim::mi_process_attach()` at the top. That makes the test pass but leaves public API initialization broken.
- Do not force these allocations to use arenas by manually reserving memory in the test. That also hides the lifecycle regression.
- Do not remove the `_mi_preloading()` checks from arena allocation or purge. Those checks are correct for true preload state; the bug is that normal initialization can leave the process in that state indefinitely.
