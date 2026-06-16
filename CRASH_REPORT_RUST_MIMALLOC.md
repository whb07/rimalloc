# Rust Mimalloc Crash Report: Signal 6 Aborts

## Executive Summary
During the performance benchmarking of the Rust port of `mimalloc` against the original C implementation, several multi-threaded and cache-intensive C++ benchmarks repeatedly crashed with **Signal 6 (SIGABRT)**. 

The affected tests included:
- `larsonN-sized`
- `gs`
- `alloc-test1` and `alloc-testN`
- `cache-scratch1` and `cache-scratchN`

**Status:** **Resolved**. The crashes were traced to a logic error in the naked assembly fast-path override for the C++ `operator new` (`_Znwm`).

---

## Investigation Details

### 1. Reproduction & Symptoms
The aborts only occurred under specific conditions:
- **Release Profile Only:** The benchmarks completed successfully when compiled in `dev` (debug) mode, but immediately failed in `release` mode.
- **Silent Abort:** No standard Rust panic traces or mimalloc error messages were emitted to `stderr` prior to the abort. 
- **Backtrace:** Running `gdb` on the core dumps pointed to `mechanical_mimalloc::alloc::mi_theap_try_new()` as the function triggering `abort()`.

### 2. Root Cause Analysis
The C++ `operator new` (mangled as `_Znwm` on Linux x86_64) is overridden by mimalloc to route C++ allocations into the mimalloc heap. 

In `--release` builds targeting Linux x86_64 without security features enabled, `mechanical_mimalloc` compiles an ultra-fast allocation path written in naked assembly (`mi_cxx_new_fast_64` in `src/alloc_override.rs`).

If the fast-path fails (e.g., if there are no free blocks immediately available in the thread-local page cache, or if the requested size exceeds the fast-path limit), the assembly defers to a slow-path handler via a `jmp` instruction:
```nasm
"2:"
"xor edx, edx"
"jmp {try_new}"
try_new = sym crate::alloc::mi_theap_try_new,
```

**The Bug:** The assembly was jumping directly to `mi_theap_try_new`. 
By design, `mi_theap_try_new` expects that a standard allocation attempt (`mi_theap_malloc`) has **already been made and failed**. Its implementation initializes a pointer to `null`, detects the failure state, and immediately invokes the C++ `new_handler` (`mi_try_new_handler`):
```rust
pub(crate) unsafe extern "C" fn mi_theap_try_new(theap: *mut mi_theap_t, size: usize, nothrow: bool) -> *mut c_void {
    let mut p: *mut c_void = ptr::null_mut();
    while p.is_null() && mi_try_new_handler(nothrow) { ... }
    p
}
```
Because no custom `new_handler` is set in these benchmarks, `mi_try_new_handler` printed an internally suppressed "out of memory in 'new'" warning and immediately invoked `abort()`. The allocator was never actually out of memory; the fast-path simply skipped the slow-path allocation logic altogether.

### 3. Resolution
The assembly jump target was corrected to point to `mi_theap_alloc_new`.

1. **Expose Slow Path:** `mi_theap_alloc_new` in `src/alloc.rs` was exposed as an `extern "C"` function so the assembly could safely target it.
2. **Update Assembly:** The `jmp {try_new}` instruction in `src/alloc_override.rs` was redirected to `jmp {alloc_new}`.

```rust
// In src/alloc_override.rs
        "2:",
        "xor edx, edx",
        "jmp {alloc_new}",
        alloc_new = sym crate::alloc::mi_theap_alloc_new,
```

When the fast-path fails, execution now correctly routes to `mi_theap_alloc_new`, which attempts a standard heap allocation (`mi_theap_malloc`) and only falls back to the throwing/aborting behavior of `mi_theap_try_new` if the slow path actually exhausts available memory.

## Verification
Following the patch, all previously crashing benchmarks (`alloc-test`, `cache-scratch`, `larson-sized`) were rebuilt in release mode and verified to complete successfully without aborts.
