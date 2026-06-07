#[path = "original_testhelper.rs"]
mod original_testhelper;

use std::slice;

use mechanical_mimalloc::alloc::{
    mi_calloc, mi_malloc_small, mi_mallocn, mi_reallocn, mi_recalloc, mi_rezalloc, mi_zalloc,
    mi_zalloc_small,
};
use mechanical_mimalloc::alloc_aligned::{
    mi_calloc_aligned, mi_malloc_aligned, mi_realloc_aligned, mi_recalloc_aligned,
    mi_rezalloc_aligned, mi_zalloc_aligned,
};
use mechanical_mimalloc::free::mi_free;
use mechanical_mimalloc::internal::MI_DEBUG;
use mechanical_mimalloc::options::{mi_option_disable, mi_option_t};
use mechanical_mimalloc::track::{MI_TRACK_ASAN, MI_TRACK_VALGRIND};
use mechanical_mimalloc::types::{
    MI_DEBUG_FREED, MI_DEBUG_UNINIT, MI_MAX_ALIGN_SIZE, MI_SMALL_SIZE_MAX,
};

fn check_zero_init(p: *const u8, size: usize) -> bool {
    if p.is_null() {
        return false;
    }
    unsafe { slice::from_raw_parts(p, size) }
        .iter()
        .all(|byte| *byte == 0)
}

fn check_debug_fill_uninit(p: *const u8, size: usize) -> bool {
    if MI_TRACK_VALGRIND != 0 || MI_TRACK_ASAN != 0 {
        return true;
    }
    if p.is_null() {
        return false;
    }
    unsafe { slice::from_raw_parts(p, size) }
        .iter()
        .all(|byte| *byte == MI_DEBUG_UNINIT)
}

fn check_debug_fill_freed(p: *const u8, size: usize) -> bool {
    if MI_TRACK_VALGRIND != 0 {
        return true;
    }
    if p.is_null() {
        return false;
    }
    unsafe { slice::from_raw_parts(p, size) }
        .iter()
        .all(|byte| *byte == MI_DEBUG_FREED)
}

unsafe fn run_zero_case<F>(name: &str, size: usize, alloc_fn: F)
where
    F: Fn(usize) -> *mut core::ffi::c_void,
{
    let p = alloc_fn(size).cast::<u8>();
    let result = check_zero_init(p, size);
    let _ = original_testhelper::check(name, result);
    unsafe { mi_free(p.cast()) };
}

unsafe fn run_grow_zero_case<F, G>(name: &str, initial: usize, alloc_fn: F, grow_fn: G)
where
    F: Fn(usize) -> *mut core::ffi::c_void,
    G: Fn(*mut core::ffi::c_void, usize) -> *mut core::ffi::c_void,
{
    let p = alloc_fn(initial).cast::<u8>();
    let mut result = check_zero_init(p, initial);
    let grown = grow_fn(p.cast(), initial * 3).cast::<u8>();
    result &= check_zero_init(grown, initial * 3);
    let _ = original_testhelper::check(name, result);
    unsafe { mi_free(grown.cast()) };
}

unsafe fn run_aligned_zero_case<F>(name: &str, size: usize, alloc_fn: F)
where
    F: Fn(usize, usize) -> *mut core::ffi::c_void,
{
    let alignment = MI_MAX_ALIGN_SIZE * 2;
    let p = alloc_fn(size, alignment).cast::<u8>();
    let result = check_zero_init(p, size);
    let _ = original_testhelper::check(name, result);
    unsafe { mi_free(p.cast()) };
}

unsafe fn run_aligned_grow_zero_case<F, G>(name: &str, initial: usize, grow_fn: F, alloc_fn: G)
where
    F: Fn(*mut core::ffi::c_void, usize, usize) -> *mut core::ffi::c_void,
    G: Fn(usize, usize) -> *mut core::ffi::c_void,
{
    let alignment = MI_MAX_ALIGN_SIZE * 2;
    let p = alloc_fn(initial, alignment).cast::<u8>();
    let mut result = check_zero_init(p, initial);
    let grown = grow_fn(p.cast(), initial * 3, alignment).cast::<u8>();
    result &= check_zero_init(grown, initial * 3);
    let _ = original_testhelper::check(name, result);
    unsafe { mi_free(grown.cast()) };
}

fn original_test_api_fill_main() -> i32 {
    original_testhelper::reset_test_summary();
    mi_option_disable(mi_option_t::mi_option_verbose);

    unsafe {
        let small = MI_SMALL_SIZE_MAX / 2;
        let large = MI_SMALL_SIZE_MAX * 2;

        run_zero_case("zeroinit-zalloc-small", small, |size| mi_zalloc(size));
        run_zero_case("zeroinit-zalloc-large", large, |size| mi_zalloc(size));
        run_zero_case("zeroinit-zalloc_small", small, |size| mi_zalloc_small(size));

        run_zero_case("zeroinit-calloc-small", small, |size| mi_calloc(size, 1));
        run_zero_case("zeroinit-calloc-large", large, |size| mi_calloc(size, 1));

        run_grow_zero_case(
            "zeroinit-rezalloc-small",
            small,
            |size| mi_zalloc(size),
            |p, size| mi_rezalloc(p, size),
        );
        run_grow_zero_case(
            "zeroinit-rezalloc-large",
            large,
            |size| mi_zalloc(size),
            |p, size| mi_rezalloc(p, size),
        );
        run_grow_zero_case(
            "zeroinit-recalloc-small",
            small,
            |size| mi_calloc(size, 1),
            |p, size| mi_recalloc(p, size, 1),
        );
        run_grow_zero_case(
            "zeroinit-recalloc-large",
            large,
            |size| mi_calloc(size, 1),
            |p, size| mi_recalloc(p, size, 1),
        );

        run_aligned_zero_case("zeroinit-zalloc_aligned-small", small, |size, alignment| {
            mi_zalloc_aligned(size, alignment)
        });
        run_aligned_zero_case("zeroinit-zalloc_aligned-large", large, |size, alignment| {
            mi_zalloc_aligned(size, alignment)
        });
        run_aligned_zero_case("zeroinit-calloc_aligned-small", small, |size, alignment| {
            mi_calloc_aligned(size, 1, alignment)
        });
        run_aligned_zero_case("zeroinit-calloc_aligned-large", large, |size, alignment| {
            mi_calloc_aligned(size, 1, alignment)
        });

        run_aligned_grow_zero_case(
            "zeroinit-rezalloc_aligned-small",
            small,
            |p, size, alignment| mi_rezalloc_aligned(p, size, alignment),
            |size, alignment| mi_zalloc_aligned(size, alignment),
        );
        run_aligned_grow_zero_case(
            "zeroinit-rezalloc_aligned-large",
            large,
            |p, size, alignment| mi_rezalloc_aligned(p, size, alignment),
            |size, alignment| mi_zalloc_aligned(size, alignment),
        );
        run_aligned_grow_zero_case(
            "zeroinit-recalloc_aligned-small",
            small,
            |p, size, alignment| mi_recalloc_aligned(p, size, 1, alignment),
            |size, alignment| mi_calloc_aligned(size, 1, alignment),
        );
        run_aligned_grow_zero_case(
            "zeroinit-recalloc_aligned-large",
            large,
            |p, size, alignment| mi_recalloc_aligned(p, size, 1, alignment),
            |size, alignment| mi_calloc_aligned(size, 1, alignment),
        );

        if MI_DEBUG >= 2 {
            let p = mi_malloc_small(small).cast::<u8>();
            let _ = original_testhelper::check(
                "uninit-malloc_small",
                check_debug_fill_uninit(p, small),
            );
            mi_free(p.cast());

            let p = mi_mallocn(small, 1).cast::<u8>();
            let _ = original_testhelper::check(
                "uninit-mallocn-small",
                check_debug_fill_uninit(p, small),
            );
            let p = mi_reallocn(p.cast(), small * 3, 1).cast::<u8>();
            let _ = original_testhelper::check(
                "uninit-reallocn-small",
                check_debug_fill_uninit(p, small * 3),
            );
            mi_free(p.cast());

            let p = mi_malloc_aligned(small, MI_MAX_ALIGN_SIZE * 2).cast::<u8>();
            let _ = original_testhelper::check(
                "uninit-malloc_aligned-small",
                check_debug_fill_uninit(p, small),
            );
            let p = mi_realloc_aligned(p.cast(), small * 3, MI_MAX_ALIGN_SIZE * 2).cast::<u8>();
            let _ = original_testhelper::check(
                "uninit-realloc_aligned-small",
                check_debug_fill_uninit(p, small * 3),
            );
            mi_free(p.cast());

            if MI_TRACK_VALGRIND == 0 {
                let p = mi_malloc_small(small).cast::<u8>();
                mi_free(p.cast());
                let _ = original_testhelper::check(
                    "fill-freed-small",
                    check_debug_fill_freed(
                        p.wrapping_add(core::mem::size_of::<usize>()),
                        small - core::mem::size_of::<usize>(),
                    ),
                );
            }
        }
    }

    original_testhelper::print_test_summary()
}

#[test]
fn original_test_api_fill_harness() {
    assert_eq!(original_test_api_fill_main(), 0);
}
