use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::ffi::c_void;
use std::alloc::System;
use std::sync::atomic::{AtomicUsize, Ordering};

use mechanical_mimalloc::raw::mimalloc::*;

struct DetectingAllocator;

thread_local! {
    static IN_CRITICAL_PATH: Cell<bool> = const { Cell::new(false) };
}

static GLOBAL_ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for DetectingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_global_allocator_call();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record_global_allocator_call();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_global_allocator_call();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: DetectingAllocator = DetectingAllocator;

fn record_global_allocator_call() {
    if IN_CRITICAL_PATH.with(Cell::get) {
        GLOBAL_ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
    }
}

struct CriticalPathGuard;

impl CriticalPathGuard {
    fn enter() -> Self {
        IN_CRITICAL_PATH.with(|flag| flag.set(true));
        Self
    }
}

impl Drop for CriticalPathGuard {
    fn drop(&mut self) {
        IN_CRITICAL_PATH.with(|flag| flag.set(false));
    }
}

fn assert_no_global_allocations(name: &str, f: unsafe fn() -> bool) {
    IN_CRITICAL_PATH.with(|flag| flag.set(false));
    GLOBAL_ALLOC_CALLS.store(0, Ordering::Relaxed);

    let ok = {
        let _guard = CriticalPathGuard::enter();
        unsafe { f() }
    };

    assert!(ok, "{name} failed");
    assert_eq!(GLOBAL_ALLOC_CALLS.load(Ordering::Relaxed), 0, "{name}");
}

unsafe fn malloc_free_roundtrip() -> bool {
    let p = unsafe { mi_malloc(64) };
    if p.is_null() {
        return false;
    }
    unsafe { mi_free(p) };
    true
}

unsafe fn small_malloc_free_roundtrip() -> bool {
    let p = unsafe { mi_malloc_small(32) };
    if p.is_null() {
        return false;
    }
    unsafe { mi_free_small(p) };
    true
}

unsafe fn aligned_malloc_free_roundtrip() -> bool {
    let p = unsafe { mi_malloc_aligned(128, 64) };
    if p.is_null() || !(p as usize).is_multiple_of(64) {
        return false;
    }
    unsafe { mi_free(p) };
    true
}

unsafe fn realloc_roundtrip() -> bool {
    let p = unsafe { mi_malloc(32) };
    if p.is_null() {
        return false;
    }
    let q = unsafe { mi_rezalloc(p, 96) };
    if q.is_null() {
        unsafe { mi_free(p) };
        return false;
    }
    unsafe { mi_free(q) };
    true
}

unsafe fn calloc_free_roundtrip() -> bool {
    let p = unsafe { mi_calloc(4, 16) };
    if p.is_null() {
        return false;
    }
    unsafe { mi_free(p) };
    true
}

unsafe fn zalloc_aligned_free_roundtrip() -> bool {
    let p = unsafe { mi_zalloc_aligned(128, 64) };
    if p.is_null() || !(p as usize).is_multiple_of(64) {
        return false;
    }
    unsafe { mi_free(p) };
    true
}

unsafe fn heap_lifecycle_roundtrip() -> bool {
    let heap = unsafe { mi_heap_new() };
    if heap.is_null() {
        return false;
    }
    let p = unsafe { mi_heap_malloc(heap, 80) };
    if p.is_null() {
        unsafe { mi_heap_delete(heap) };
        return false;
    }
    unsafe {
        mi_free(p.cast::<c_void>());
        mi_heap_delete(heap);
    }
    true
}

unsafe fn heap_lookup_roundtrip() -> bool {
    let heap = unsafe { mi_heap_new() };
    if heap.is_null() {
        return false;
    }
    let p = unsafe { mi_heap_malloc(heap, 80) };
    if p.is_null() {
        unsafe { mi_heap_delete(heap) };
        return false;
    }

    let theap = unsafe { mi_heap_theap(heap) };
    let contains = unsafe { mi_heap_contains(heap, p) };
    let any = unsafe { mi_any_heap_contains(p) };
    let under_utilized = unsafe { mi_unsafe_heap_page_is_under_utilized(heap, p, 100) };
    core::hint::black_box(under_utilized);

    unsafe {
        mi_free(p.cast::<c_void>());
        mi_heap_delete(heap);
    }
    !theap.is_null() && contains && any
}

#[test]
fn allocator_critical_paths_do_not_use_rust_global_allocator() {
    unsafe {
        mi_process_init();
        let warmup = mi_malloc(16);
        assert!(!warmup.is_null());
        mi_free(warmup);
    }

    assert_no_global_allocations("mi_malloc/mi_free", malloc_free_roundtrip);
    assert_no_global_allocations("mi_malloc_small/mi_free_small", small_malloc_free_roundtrip);
    assert_no_global_allocations("mi_malloc_aligned/mi_free", aligned_malloc_free_roundtrip);
    assert_no_global_allocations("mi_calloc/mi_free", calloc_free_roundtrip);
    assert_no_global_allocations("mi_rezalloc/mi_free", realloc_roundtrip);
    assert_no_global_allocations("mi_zalloc_aligned/mi_free", zalloc_aligned_free_roundtrip);
    assert_no_global_allocations("mi_heap_new/mi_heap_delete", heap_lifecycle_roundtrip);
    assert_no_global_allocations("heap lookup helpers", heap_lookup_roundtrip);
}
