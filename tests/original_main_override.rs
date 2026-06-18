#[path = "original_testhelper.rs"]
mod original_testhelper;

use core::ffi::c_void;
use std::ffi::{CStr, CString};
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicI64, AtomicPtr, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use mechanical_mimalloc::raw::{self, headers};

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn realloc(p: *mut c_void, size: usize) -> *mut c_void;
}

static STATIC_P: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static SHARED_P: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static GLOBAL_P: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static CPTR: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static XGSUM: AtomicI64 = AtomicI64::new(0);
static GSUM: AtomicI64 = AtomicI64::new(0);
static GSUM5: AtomicI64 = AtomicI64::new(0);
static SUM2: AtomicI64 = AtomicI64::new(0);
static TEST_MUTEX: Mutex<()> = Mutex::new(());

const LEN: [usize; 4] = [1000, 5000, 10_000, 50_000];
const THREAD_LEAK_ITERS: usize = 256;
const PERF_ITERS: usize = 256;
const PERF2_ITERS: usize = 128;
const PERF3_ITERS: usize = 2;
const PERF4_ITERS: usize = 256;
const PERF5_ITERS: usize = 512;

#[derive(Default)]
struct SomeStruct {
    i: i32,
    j: i32,
    z: f64,
}

struct Test {
    i: i32,
}

impl Test {
    fn new(i: i32) -> Self {
        Self { i }
    }
}

struct MTest {
    data: *mut c_void,
}

impl MTest {
    fn new() -> Self {
        let data = unsafe { malloc(1024) };
        assert!(!data.is_null());
        Self { data }
    }
}

impl Drop for MTest {
    fn drop(&mut self) {
        unsafe { free(self.data) };
    }
}

thread_local! {
    static TL_VARIABLE: MTest = MTest::new();
    static S_PTR: *mut c_void = unsafe { raw::mi_malloc(1) };
}

fn msleep(msecs: u64) {
    thread::sleep(Duration::from_millis(msecs));
}

fn test_guard() -> MutexGuard<'static, ()> {
    TEST_MUTEX.lock().expect("test mutex poisoned")
}

fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    *state
}

fn ensure_static_p() {
    if !STATIC_P.load(Ordering::Acquire).is_null() {
        return;
    }
    let p = unsafe { malloc(8) };
    assert!(!p.is_null());
    if STATIC_P
        .compare_exchange(ptr::null_mut(), p, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        unsafe { free(p) };
    }
}

fn original_main_override_c_main() -> i32 {
    original_testhelper::reset_test_summary();

    unsafe {
        let mut p1 = malloc(78);
        let in_heap_1 = !p1.is_null() && raw::mi_is_in_heap_region(p1.cast_const());
        let _ = original_testhelper::check("main-override.c:malloc78", in_heap_1);

        let mut p2 = malloc(24);
        let in_heap_2 = !p2.is_null() && raw::mi_is_in_heap_region(p2.cast_const());
        let _ = original_testhelper::check("main-override.c:malloc24", in_heap_2);

        free(p1);
        p1 = malloc(8);

        let hello = CString::new("hello\n").expect("CString literal");
        let s = raw::mi_strdup(hello.as_ptr());
        let strdup_ok = if s.is_null() {
            false
        } else {
            CStr::from_ptr(s) == hello.as_c_str()
        };
        let _ = original_testhelper::check("main-override.c:strdup", strdup_ok);

        free(p2);
        p2 = malloc(16);
        let p3 = realloc(p1, 32);
        if !p3.is_null() {
            p1 = p3;
        }
        free(p1);
        free(p2);
        raw::mi_free(s.cast());

        p1 = raw::mi_malloc(32);
        free(p1);
        p2 = malloc(32);
        raw::mi_free(p2);

        let _ = original_testhelper::check("main-override.c:mi_version", raw::mi_version() > 0);
        raw::mi_stats_print(ptr::null_mut());
    }

    original_testhelper::print_test_summary()
}

fn free_p() {
    let p = STATIC_P.swap(ptr::null_mut(), Ordering::AcqRel);
    if !p.is_null() {
        unsafe { free(p) };
    }
}

fn various_tests() {
    ensure_static_p();

    unsafe {
        let mut p1 = malloc(78);
        let p2 = raw::mi_malloc_aligned(24, 16);
        assert!(!p1.is_null());
        assert!(!p2.is_null());
        free(p1);

        p1 = malloc(8);
        let hello = CString::new("hello\n").expect("CString literal");
        let s = raw::mi_strdup(hello.as_ptr());
        assert!(!s.is_null());

        raw::mi_free(p2);
        let p2 = malloc(16);
        assert!(!p2.is_null());

        let p3 = realloc(p1, 32);
        if !p3.is_null() {
            p1 = p3;
        }
        free(p1);
        free(p2);
        raw::mi_free(s.cast());

        let t = Box::new(Test::new(42));
        assert_eq!(t.i, 42);
        drop(t);

        let t2 = raw::mi_new_nothrow(mem::size_of::<Test>()).cast::<Test>();
        if !t2.is_null() {
            ptr::write(t2, Test::new(42));
            assert_eq!((*t2).i, 42);
            ptr::drop_in_place(t2);
            raw::mi_free(t2.cast());
        }

        let tbuf = raw::mi_new_n(1, mem::size_of::<Test>()).cast::<u8>();
        assert!(!tbuf.is_null());
        let t3 = tbuf.cast::<Test>();
        ptr::write(t3, Test::new(42));
        assert_eq!((*t3).i, 42);
        ptr::drop_in_place(t3);
        raw::mi_free(tbuf.cast());
    }

    free_p();
}

fn test_stl_allocator1() -> bool {
    let _ = core::hint::black_box((
        headers::mi_stl_allocator,
        headers::mi_stl_allocator_operator_eq,
        headers::mi_stl_allocator_operator_ne,
    ));

    let mut vec = Vec::<i32>::new();
    vec.push(1);
    vec.pop();
    vec.is_empty()
}

fn test_dep() {
    #[cfg(windows)]
    {
        let s = CString::new("test_dep GetString").expect("CString literal");
        let dup = unsafe { raw::mi_strdup(s.as_ptr()) };
        assert!(!dup.is_null());
        unsafe { raw::mi_free(dup.cast()) };

        let test_heap = unsafe { raw::mi_heap_new() };
        assert!(!test_heap.is_null());
        let p = unsafe { raw::mi_heap_malloc(test_heap, 64) };
        assert!(!p.is_null());
        unsafe {
            raw::mi_free(p);
            raw::mi_heap_delete(test_heap);
        }
    }
}

fn test_stl_allocator2() -> bool {
    core::hint::black_box(headers::mi_stl_allocator);

    let item = SomeStruct::default();
    let _ = item.i + item.j + item.z as i32;

    let mut vec = Vec::<SomeStruct>::new();
    vec.push(item);
    vec.pop();
    vec.is_empty()
}

fn test_stl_allocator3() -> bool {
    core::hint::black_box(headers::mi_heap_stl_allocator);

    let mut vec = Vec::<i32>::new();
    vec.push(1);
    vec.pop();
    vec.is_empty()
}

fn test_stl_allocator4() -> bool {
    let _ = core::hint::black_box((
        headers::mi_heap_stl_allocator,
        headers::mi_heap_stl_allocator_operator_eq,
        headers::mi_heap_stl_allocator_operator_ne,
    ));

    let item = SomeStruct::default();
    let _ = item.i + item.j + item.z as i32;

    let mut vec = Vec::<SomeStruct>::new();
    vec.push(item);
    vec.pop();
    vec.is_empty()
}

fn test_stl_allocator5() -> bool {
    core::hint::black_box(headers::mi_heap_destroy_stl_allocator);

    let mut vec = Vec::<i32>::new();
    vec.push(1);
    vec.pop();
    vec.is_empty()
}

fn test_stl_allocator6() -> bool {
    let _ = core::hint::black_box((
        headers::mi_heap_destroy_stl_allocator,
        headers::mi_heap_destroy_stl_allocator_operator_eq,
        headers::mi_heap_destroy_stl_allocator_operator_ne,
    ));

    let item = SomeStruct::default();
    let _ = item.i + item.j + item.z as i32;

    let mut vec = Vec::<SomeStruct>::new();
    vec.push(item);
    vec.pop();
    vec.is_empty()
}

fn test_stl_allocators() {
    assert!(test_stl_allocator1());
    assert!(test_stl_allocator2());
    if raw::MI_HAS_HEAP_STL_ALLOCATOR != 0 {
        assert!(test_stl_allocator3());
        assert!(test_stl_allocator4());
        assert!(test_stl_allocator5());
        assert!(test_stl_allocator6());
    }
}

fn thread1_allocate() {
    let p = unsafe { raw::mi_calloc(1, 256) };
    assert!(!p.is_null());
    let old = CPTR.swap(p, Ordering::AcqRel);
    if !old.is_null() {
        unsafe { raw::mi_free(old) };
    }
}

fn thread2_free() {
    let p = CPTR.swap(ptr::null_mut(), Ordering::AcqRel);
    if !p.is_null() {
        unsafe { raw::mi_free(p) };
    }
}

fn test_large_migrate() {
    let t1 = thread::spawn(thread1_allocate);
    t1.join().expect("thread1_allocate");
    let t2 = thread::spawn(thread2_free);
    t2.join().expect("thread2_free");
}

fn strdup_test() {
    let s = CString::new("hello\n").expect("CString literal");
    let dup = unsafe { raw::mi_strdup(s.as_ptr()) };
    assert!(!dup.is_null());
    unsafe { raw::mi_free(dup.cast()) };

    #[cfg(windows)]
    {
        let _ = std::env::var("PATH").ok();
    }
}

fn heap_no_delete() {
    let t = thread::spawn(|| unsafe {
        let test_heap = raw::mi_heap_new();
        assert!(!test_heap.is_null());
        let q = raw::mi_heap_malloc(test_heap, 1024);
        assert!(!q.is_null());
        let _ = core::hint::black_box(q);
    });
    t.join().expect("heap_no_delete_worker");
}

fn t1main() {
    unsafe {
        let test_heap = raw::mi_heap_new();
        assert!(!test_heap.is_null());
        let p = raw::mi_heap_malloc(test_heap, 1024);
        assert!(!p.is_null());
        GLOBAL_P.store(p, Ordering::Release);
        raw::mi_heap_delete(test_heap);
    }
}

fn theap_late_free() {
    let t = thread::spawn(t1main);
    msleep(2000);
    let p = GLOBAL_P.swap(ptr::null_mut(), Ordering::AcqRel);
    assert!(!p.is_null());
    unsafe { raw::mi_free(p) };
    t.join().expect("t1main");
}

fn alloc0() {
    let p = unsafe { raw::mi_malloc(8) };
    assert!(!p.is_null());
    let old = SHARED_P.swap(p, Ordering::AcqRel);
    if !old.is_null() {
        unsafe { raw::mi_free(old) };
    }
}

fn padding_shrink() {
    let t = thread::spawn(alloc0);
    t.join().expect("alloc0");
    let p = SHARED_P.swap(ptr::null_mut(), Ordering::AcqRel);
    assert!(!p.is_null());
    unsafe { raw::mi_free(p) };
}

fn theap_thread_free_large() {
    for _ in 0..4 {
        let p = unsafe { raw::mi_malloc_aligned(2 * 1024 * 1024 + 1, 8) };
        assert!(!p.is_null());
        let old = SHARED_P.swap(p, Ordering::AcqRel);
        if !old.is_null() {
            unsafe { raw::mi_free(old) };
        }
        let t = thread::spawn(|| {
            let p = SHARED_P.swap(ptr::null_mut(), Ordering::AcqRel);
            if !p.is_null() {
                unsafe { raw::mi_free(p) };
            }
        });
        t.join().expect("theap_thread_free_large_worker");
    }
}

fn theap_thread_free_huge_worker() {
    let p = SHARED_P.swap(ptr::null_mut(), Ordering::AcqRel);
    if !p.is_null() {
        unsafe { raw::mi_free(p) };
    }
}

fn theap_thread_free_huge_smoke() {
    let p = unsafe { raw::mi_malloc(4096) };
    assert!(!p.is_null());
    let old = SHARED_P.swap(p, Ordering::AcqRel);
    if !old.is_null() {
        unsafe { raw::mi_free(old) };
    }
    let t = thread::spawn(theap_thread_free_huge_worker);
    t.join().expect("theap_thread_free_huge_worker");
}

fn local_alloc() {
    let mut state = 0x1234_5678_9abc_def0_u64;
    let mut sum = 0_i64;
    for _ in 0..THREAD_LEAK_ITERS {
        let n = 1 + (lcg_next(&mut state) as usize % 1000);
        let p = unsafe { calloc(n, 1).cast::<u8>() };
        assert!(!p.is_null());
        unsafe {
            *p = 1;
            sum += i64::from(*p.add((lcg_next(&mut state) as usize) % n));
        }
        if lcg_next(&mut state) % 100 > 24 {
            unsafe { free(p.cast()) };
        }
    }
    XGSUM.fetch_add(sum, Ordering::Relaxed);
}

fn test_thread_leak() {
    let mut threads = Vec::new();
    for _ in 0..8 {
        threads.push(thread::spawn(local_alloc));
    }
    for handle in threads {
        handle.join().expect("local_alloc");
    }
}

fn test_mt_shutdown() {
    let mut tasks = Vec::new();
    for _ in 0..4 {
        tasks.push(thread::spawn(|| {
            let mut ps = Vec::with_capacity(64);
            for _ in 0..64 {
                let p = unsafe { raw::mi_new_n(1, 1) };
                assert!(!p.is_null());
                ps.push(p as usize);
            }
            ps
        }));
    }

    for task in tasks {
        for p in task.join().expect("test_mt_shutdown worker") {
            unsafe { raw::mi_free(p as *mut c_void) };
        }
    }
}

fn thread_fun(i: i32) {
    TL_VARIABLE.with(|v| {
        let _ = core::hint::black_box(v.data);
    });
    let _ = core::hint::black_box(i);
    thread::sleep(Duration::from_millis(1));
}

fn test_perf_local_alloc() {
    let mut state = 0x1357_2468_u64;
    let mut sum = 0_i64;
    for _ in 0..PERF_ITERS {
        let len = LEN[(lcg_next(&mut state) as usize) % LEN.len()];
        let p = unsafe {
            raw::mi_zalloc_aligned(len * mem::size_of::<i32>(), mem::align_of::<i32>())
                .cast::<i32>()
        };
        assert!(!p.is_null());
        unsafe {
            *p = 1;
            sum += i64::from(*p.add((lcg_next(&mut state) as usize) % len));
            raw::mi_free(p.cast());
        }
    }
    GSUM.fetch_add(sum, Ordering::Relaxed);
}

fn test_perf_run() {
    let mut threads = Vec::new();
    for _ in 0..4 {
        threads.push(thread::spawn(test_perf_local_alloc));
    }
    for handle in threads {
        handle.join().expect("test_perf_local_alloc");
    }
}

fn escape(p: *mut u8, n: usize) {
    if p.is_null() || n == 0 {
        return;
    }
    unsafe {
        *p.add(n / 2) = 42;
        SUM2.fetch_add(i64::from(*p.add(n / 2)), Ordering::Relaxed);
    }
}

fn test_perf2() {
    for _ in 0..PERF2_ITERS {
        let n = 1000;
        let p = unsafe { calloc(1, n).cast::<u8>() };
        assert!(!p.is_null());
        escape(p, n);
        unsafe { free(p.cast()) };
    }
}

fn test_perf3() {
    for _ in 0..PERF3_ITERS {
        let n = 64 * 1024;
        let p = unsafe { calloc(1, n).cast::<u8>() };
        assert!(!p.is_null());
        escape(p, n);
        unsafe { free(p.cast()) };
    }
}

fn local_alloc4() {
    for i in 0..PERF4_ITERS {
        let n = i % 1000;
        let p = unsafe { calloc(1, n.max(1)).cast::<u8>() };
        assert!(!p.is_null());
        escape(p, n);
        if i % 4 > 0 {
            unsafe { free(p.cast()) };
        }
    }
}

fn test_perf4() {
    let mut threads = Vec::new();
    for _ in 0..8 {
        threads.push(thread::spawn(local_alloc4));
    }
    for handle in threads {
        handle.join().expect("local_alloc4");
    }
}

fn escape5(p: *mut u8, n: usize) {
    if p.is_null() || n == 0 {
        return;
    }
    unsafe {
        for i in 0..n {
            *p.add(i) = (i & 0xFF) as u8;
        }
        *p.add(n / 2) = (n & 0xFF) as u8;
    }
}

fn local_alloc5() {
    let mut sum = 0_i64;
    for i in 0..PERF5_ITERS {
        let n = i % 1000;
        let p = unsafe { raw::mi_malloc(n.max(1)).cast::<u8>() };
        assert!(!p.is_null());
        escape5(p, n);
        if i % 4 > 0 {
            if n > 0 {
                unsafe {
                    sum += i64::from(*p.add(n - 1));
                }
            }
            unsafe { raw::mi_free(p.cast()) };
        }
    }
    GSUM5.fetch_add(sum, Ordering::Relaxed);
}

fn test_perf5() {
    let mut threads = Vec::new();
    for _ in 0..8 {
        threads.push(thread::spawn(local_alloc5));
    }
    for handle in threads {
        handle.join().expect("local_alloc5");
    }
}

fn original_main_override_cpp_main() -> i32 {
    unsafe { raw::mi_stats_reset() };
    various_tests();
    test_dep();
    test_large_migrate();
    strdup_test();
    heap_no_delete();
    theap_late_free();
    padding_shrink();
    theap_thread_free_large();
    thread_fun(1);
    test_stl_allocators();
    test_thread_leak();
    test_mt_shutdown();
    test_perf_run();
    test_perf2();
    test_perf3();
    test_perf4();
    test_perf5();
    theap_thread_free_huge_smoke();
    unsafe { raw::mi_stats_print(ptr::null_mut()) };
    0
}

#[test]
fn original_main_override_c_harness() {
    let _guard = test_guard();
    assert_eq!(original_main_override_c_main(), 0);
}

#[test]
fn original_main_override_cpp_harness() {
    let _guard = test_guard();
    assert_eq!(original_main_override_cpp_main(), 0);
}

#[test]
fn original_main_override_test_stl_allocator3() {
    let _guard = test_guard();
    assert!(test_stl_allocator3());
}

#[test]
fn original_main_override_test_stl_allocator4() {
    let _guard = test_guard();
    assert!(test_stl_allocator4());
}

#[test]
fn original_main_override_test_stl_allocator5() {
    let _guard = test_guard();
    assert!(test_stl_allocator5());
}

#[test]
fn original_main_override_test_stl_allocator6() {
    let _guard = test_guard();
    assert!(test_stl_allocator6());
}

#[test]
fn original_main_override_test_dep() {
    let _guard = test_guard();
    test_dep();
}

#[test]
fn original_main_override_test_large_migrate() {
    let _guard = test_guard();
    test_large_migrate();
}

#[test]
fn original_main_override_strdup_test() {
    let _guard = test_guard();
    strdup_test();
}

#[test]
fn original_main_override_heap_no_delete() {
    let _guard = test_guard();
    heap_no_delete();
}

#[test]
fn original_main_override_theap_late_free() {
    let _guard = test_guard();
    theap_late_free();
}

#[test]
fn original_main_override_padding_shrink() {
    let _guard = test_guard();
    padding_shrink();
}

#[test]
fn original_main_override_theap_thread_free_large() {
    let _guard = test_guard();
    theap_thread_free_large();
}

#[test]
fn original_main_override_theap_thread_free_huge_worker() {
    let _guard = test_guard();
    theap_thread_free_huge_smoke();
}

#[test]
fn original_main_override_thread_fun() {
    let _guard = test_guard();
    thread_fun(7);
}

#[test]
fn original_main_override_test_thread_leak() {
    let _guard = test_guard();
    test_thread_leak();
}

#[test]
fn original_main_override_test_mt_shutdown() {
    let _guard = test_guard();
    test_mt_shutdown();
}

#[test]
fn original_main_override_test_perf_run() {
    let _guard = test_guard();
    test_perf_run();
}

#[test]
fn original_main_override_test_perf2() {
    let _guard = test_guard();
    test_perf2();
}

#[test]
fn original_main_override_test_perf3() {
    let _guard = test_guard();
    test_perf3();
}

#[test]
fn original_main_override_test_perf4() {
    let _guard = test_guard();
    test_perf4();
}

#[test]
fn original_main_override_test_perf5() {
    let _guard = test_guard();
    test_perf5();
}
