#![allow(non_upper_case_globals)]

#[path = "original_testhelper.rs"]
mod original_testhelper;

use core::ffi::c_void;
use core::ptr;
use std::os::raw::c_long;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;

use mechanical_mimalloc::raw::mimalloc::{mi_heap_area_t, mi_heap_t, mi_theap_t};
use mechanical_mimalloc::raw::{alloc, free, heap, mimalloc, options};

const USE_HEAPS: usize = 4;
const TRANSFERS: usize = 32;
const STRESS: bool = true;

#[cfg(target_pointer_width = "64")]
const cookie: usize = 0xbf58_476d_1ce4_e5b9;
#[cfg(target_pointer_width = "32")]
const cookie: usize = 0x1ce4_e5b9;

static THREADS: AtomicUsize = AtomicUsize::new(4);
static SCALE: AtomicUsize = AtomicUsize::new(4);
static ITER: AtomicUsize = AtomicUsize::new(3);
static allow_large_objects: AtomicBool = AtomicBool::new(false);
static use_one_size: AtomicUsize = AtomicUsize::new(0);
static main_participates: AtomicBool = AtomicBool::new(false);
static current_heap: AtomicPtr<mi_heap_t> = AtomicPtr::new(ptr::null_mut());
static transfer: [AtomicPtr<c_void>; TRANSFERS] =
    [const { AtomicPtr::new(ptr::null_mut()) }; TRANSFERS];

type ThreadEntryFun = fn(isize);

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_test() -> MutexGuard<'static, ()> {
    test_lock().lock().expect("stress test lock poisoned")
}

fn reset_runtime_state() {
    THREADS.store(4, Ordering::SeqCst);
    SCALE.store(4, Ordering::SeqCst);
    ITER.store(3, Ordering::SeqCst);
    allow_large_objects.store(false, Ordering::SeqCst);
    use_one_size.store(0, Ordering::SeqCst);
    main_participates.store(false, Ordering::SeqCst);
    let active = current_heap.swap(ptr::null_mut(), Ordering::SeqCst);
    if !active.is_null() {
        unsafe { heap::mi_heap_delete(active) };
    }
    for slot in &transfer {
        let p = slot.swap(ptr::null_mut(), Ordering::SeqCst);
        if !p.is_null() {
            unsafe { free::mi_free(p) };
        }
    }
}

unsafe fn custom_calloc(count: usize, size: usize) -> *mut c_void {
    let heap_ptr = current_heap.load(Ordering::SeqCst);
    if !heap_ptr.is_null() {
        unsafe { alloc::mi_heap_calloc(heap_ptr, count, size) }
    } else {
        unsafe { alloc::mi_calloc(count, size) }
    }
}

unsafe fn custom_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    let heap_ptr = current_heap.load(Ordering::SeqCst);
    if !heap_ptr.is_null() {
        unsafe { alloc::mi_heap_realloc(heap_ptr, p, newsize) }
    } else {
        unsafe { alloc::mi_realloc(p, newsize) }
    }
}

unsafe fn custom_free(p: *mut c_void) {
    unsafe { free::mi_free(p) };
}

fn atomic_exchange_ptr(slot: &AtomicPtr<c_void>, newval: *mut c_void) -> *mut c_void {
    slot.swap(newval, Ordering::SeqCst)
}

fn pick(r: &mut usize) -> usize {
    let mut x = *r;
    #[cfg(target_pointer_width = "64")]
    {
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
    }
    #[cfg(target_pointer_width = "32")]
    {
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846c_a68b);
        x ^= x >> 16;
    }
    *r = x;
    x
}

fn chance(perc: usize, r: &mut usize) -> bool {
    pick(r) % 100 <= perc
}

unsafe fn alloc_items(mut items: usize, r: &mut usize) -> *mut c_void {
    if chance(1, r) {
        if chance(1, r) && allow_large_objects.load(Ordering::Relaxed) {
            items = items.saturating_mul(10_000);
        } else if chance(10, r) && allow_large_objects.load(Ordering::Relaxed) {
            items = items.saturating_mul(1_000);
        } else {
            items = items.saturating_mul(100);
        }
    }
    if (32..=40).contains(&items) {
        items *= 2;
    }
    let forced_size = use_one_size.load(Ordering::Relaxed);
    if forced_size > 0 {
        items = forced_size / core::mem::size_of::<usize>();
    }
    if items == 0 {
        items = 1;
    }

    let p = unsafe { custom_calloc(items, core::mem::size_of::<usize>()) }.cast::<usize>();
    if p.is_null() {
        return ptr::null_mut();
    }
    for i in 0..items {
        unsafe {
            assert_eq!(*p.add(i), 0);
            *p.add(i) = (items - i) ^ cookie;
        }
    }
    p.cast()
}

unsafe fn free_items(p: *mut c_void) {
    if !p.is_null() {
        let q = p.cast::<usize>();
        let items = unsafe { *q } ^ cookie;
        for i in 0..items {
            let expected = items - i;
            let actual = unsafe { *q.add(i) } ^ cookie;
            assert_eq!(actual, expected, "memory corruption at block {p:p} at {i}");
        }
    }
    unsafe { custom_free(p) };
}

unsafe extern "C" fn visit_blocks(
    _heap: *const mi_heap_t,
    _area: *const mi_heap_area_t,
    block: *mut c_void,
    block_size: usize,
    arg: *mut c_void,
) -> bool {
    if !block.is_null() {
        let total = unsafe { &mut *arg.cast::<usize>() };
        *total += block_size;
    }
    true
}

fn stress(tid: isize) {
    let mut r = ((tid + 1) as usize) * 43;
    let max_item_shift = 4usize;
    let max_item_retained_shift = max_item_shift + 2;
    let mut allocs = 12usize * SCALE.load(Ordering::Relaxed) * ((tid as usize % 4) + 1);
    let mut retain = allocs / 2;
    let mut data: Vec<*mut c_void> = Vec::new();
    let mut retained: Vec<*mut c_void> = Vec::new();

    while allocs > 0 || retain > 0 {
        if retain == 0 || (chance(50, &mut r) && allocs > 0) {
            allocs -= 1;
            let size_class = 1usize << (pick(&mut r) % max_item_shift);
            let p = unsafe { alloc_items(size_class, &mut r) };
            data.push(p);
        } else {
            retain -= 1;
            let size_class = 1usize << (pick(&mut r) % max_item_retained_shift);
            let p = unsafe { alloc_items(size_class, &mut r) };
            retained.push(p);
        }

        if chance(66, &mut r) && !data.is_empty() {
            let idx = pick(&mut r) % data.len();
            let p = data[idx];
            unsafe { free_items(p) };
            data[idx] = ptr::null_mut();
        }

        if chance(25, &mut r) && !data.is_empty() {
            let data_idx = pick(&mut r) % data.len();
            let transfer_idx = pick(&mut r) % TRANSFERS;
            let p = data[data_idx];
            let q = atomic_exchange_ptr(&transfer[transfer_idx], p);
            data[data_idx] = q;
        }
    }

    let mut total = 0usize;
    unsafe {
        mimalloc::mi_theap_visit_blocks(
            mimalloc::mi_theap_get_default() as *const mi_theap_t,
            true,
            Some(visit_blocks),
            (&mut total as *mut usize).cast(),
        );
    }

    for p in retained {
        unsafe { free_items(p) };
    }
    for p in data {
        unsafe { free_items(p) };
    }
}

fn leak(_tid: isize) {
    let mut r = 0x7feb_352dusize;
    let p = unsafe { alloc_items(1, &mut r) };
    if chance(50, &mut r) {
        let idx = pick(&mut r) % TRANSFERS;
        let q = atomic_exchange_ptr(&transfer[idx], p);
        unsafe { free_items(q) };
    } else {
        unsafe { free_items(p) };
    }
}

fn thread_entry(param: usize, fun: ThreadEntryFun) {
    fun(param as isize);
}

fn run_os_threads(nthreads: usize, fun: ThreadEntryFun) {
    let start = if main_participates.load(Ordering::Relaxed) {
        1usize
    } else {
        0usize
    };

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(nthreads.saturating_sub(start));
        for i in start..nthreads {
            handles.push(scope.spawn(move || thread_entry(i, fun)));
        }
        if start == 1 {
            fun(0);
        }
        for handle in handles {
            handle.join().expect("worker thread panicked");
        }
    });
}

fn test_stress() {
    let mut prev_heaps: [*mut mi_heap_t; USE_HEAPS] = [ptr::null_mut(); USE_HEAPS];
    let mut r = 0x7feb_352dusize;
    let iterations = ITER.load(Ordering::Relaxed);
    let nthreads = THREADS.load(Ordering::Relaxed);

    for n in 0..iterations {
        if !prev_heaps[USE_HEAPS - 1].is_null() {
            unsafe { heap::mi_heap_delete(prev_heaps[USE_HEAPS - 1]) };
        }
        for i in (1..USE_HEAPS).rev() {
            prev_heaps[i] = prev_heaps[i - 1];
        }

        let old = current_heap.load(Ordering::SeqCst);
        prev_heaps[0] = old;
        let new_heap = unsafe { heap::mi_heap_new() };
        assert!(!new_heap.is_null());
        current_heap.store(new_heap, Ordering::SeqCst);

        run_os_threads(nthreads, stress);

        let mut total = 0usize;
        unsafe {
            mimalloc::mi_heap_visit_blocks(
                current_heap.load(Ordering::SeqCst),
                true,
                Some(visit_blocks),
                (&mut total as *mut usize).cast(),
            );
        }

        for slot in &transfer {
            if chance(50, &mut r) || n + 1 == iterations {
                let p = atomic_exchange_ptr(slot, ptr::null_mut());
                unsafe { free_items(p) };
            }
        }
    }

    unsafe { mimalloc::mi_stats_print(ptr::null_mut()) };

    for heap_ptr in prev_heaps {
        if !heap_ptr.is_null() {
            unsafe { heap::mi_heap_delete(heap_ptr) };
        }
    }
    let active = current_heap.swap(ptr::null_mut(), Ordering::SeqCst);
    if !active.is_null() {
        unsafe { heap::mi_heap_delete(active) };
    }

    for slot in &transfer {
        let p = atomic_exchange_ptr(slot, ptr::null_mut());
        unsafe { free_items(p) };
    }
}

fn test_leak() {
    let iterations = ITER.load(Ordering::Relaxed);
    let nthreads = THREADS.load(Ordering::Relaxed);
    for _ in 0..iterations {
        run_os_threads(nthreads, leak);
        unsafe { mimalloc::mi_collect(false) };
    }
}

fn main(argc: usize, argv: &[&str]) -> i32 {
    let _guard = lock_test();
    reset_runtime_state();
    original_testhelper::reset_test_summary();

    unsafe { mimalloc::mi_version() };
    options::mi_option_enable(options::mi_option_t::mi_option_visit_abandoned);
    unsafe {
        options::mi_option_set(
            options::mi_option_t::mi_option_arena_reserve,
            (mimalloc::mi_arena_min_size() / 1024) as c_long,
        );
    }
    options::mi_option_set(options::mi_option_t::mi_option_purge_delay, 1);

    if argc >= 2
        && let Ok(n) = argv[1].parse::<usize>()
        && n > 0
    {
        THREADS.store(n, Ordering::SeqCst);
    }
    if argc >= 3
        && let Ok(n) = argv[2].parse::<usize>()
        && n > 0
    {
        SCALE.store(n, Ordering::SeqCst);
    }
    if argc >= 4
        && let Ok(n) = argv[3].parse::<usize>()
        && n > 0
    {
        ITER.store(n, Ordering::SeqCst);
    }
    if SCALE.load(Ordering::Relaxed) > 100 {
        allow_large_objects.store(true, Ordering::SeqCst);
    }

    unsafe { mimalloc::mi_stats_reset() };

    let stress_ok = std::panic::catch_unwind(|| {
        if STRESS {
            test_stress();
        } else {
            test_leak();
        }
    })
    .is_ok();
    let _ = original_testhelper::check("test-stress.c:main", stress_ok);

    unsafe {
        mimalloc::mi_collect(true);
        mimalloc::mi_stats_print(ptr::null_mut());
    }

    original_testhelper::print_test_summary()
}

#[test]
fn original_test_stress_alloc_roundtrip() {
    let _guard = lock_test();
    reset_runtime_state();
    let mut r = 43usize;
    let p = unsafe { alloc_items(8, &mut r) };
    assert!(!p.is_null());
    let p = unsafe { custom_realloc(p, 16 * core::mem::size_of::<usize>()) };
    assert!(!p.is_null());
    unsafe { free_items(p) };
}

#[test]
fn original_test_stress_leak_characterization() {
    let _guard = lock_test();
    reset_runtime_state();
    THREADS.store(2, Ordering::SeqCst);
    ITER.store(2, Ordering::SeqCst);
    test_leak();
}

#[test]
fn original_test_stress_stress_characterization() {
    let _guard = lock_test();
    reset_runtime_state();
    THREADS.store(2, Ordering::SeqCst);
    SCALE.store(2, Ordering::SeqCst);
    ITER.store(2, Ordering::SeqCst);
    test_stress();
}

#[test]
fn original_test_stress_main() {
    assert_eq!(main(4, &["stress", "2", "2", "2"]), 0);
}
