use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

static HARNESS_ALLOCS: AtomicU64 = AtomicU64::new(0);
static HARNESS_DEALLOCS: AtomicU64 = AtomicU64::new(0);

pub struct CountingSystem;

unsafe impl GlobalAlloc for CountingSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        HARNESS_ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        HARNESS_ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HARNESS_DEALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        HARNESS_ALLOCS.fetch_add(1, Ordering::Relaxed);
        HARNESS_DEALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HarnessAllocationSnapshot {
    pub allocs: u64,
    pub deallocs: u64,
}

fn snapshot() -> HarnessAllocationSnapshot {
    HarnessAllocationSnapshot {
        allocs: HARNESS_ALLOCS.load(Ordering::Relaxed),
        deallocs: HARNESS_DEALLOCS.load(Ordering::Relaxed),
    }
}

pub fn assert_no_harness_allocations<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    if std::env::var_os("MIMALLOC_BENCH_ASSERT_NO_HARNESS_ALLOC").is_none() {
        return f();
    }
    let before = snapshot();
    let result = f();
    let after = snapshot();
    assert_eq!(
        after.allocs, before.allocs,
        "timed helper allocated harness memory"
    );
    assert_eq!(
        after.deallocs, before.deallocs,
        "timed helper deallocated harness memory"
    );
    result
}
