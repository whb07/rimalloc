use core::ffi::c_void;
use core::ptr;
use std::ffi::CString;

use mechanical_mimalloc::bits::{MI_INTPTR_SIZE, MI_SIZE_BITS, mi_clz};
use mechanical_mimalloc::internal::_mi_wsize_from_size;
use mechanical_mimalloc::mimalloc;
use mechanical_mimalloc::page;
use mechanical_mimalloc::stats::MI_BIN_HUGE;
use mechanical_mimalloc::types::{mi_arena_id_t, mi_heap_area_t, mi_heap_t};

const HUGE_ALLOC_SIZE: usize = 67_108_872;
const LARGE_WSIZE_MAX: usize = (4 * 1024 * 1024) / MI_INTPTR_SIZE;

#[derive(Default)]
struct VisitState {
    areas: usize,
    blocks: usize,
}

unsafe fn alloc_huge() {
    let p = unsafe { mimalloc::mi_malloc(HUGE_ALLOC_SIZE) };
    unsafe { mimalloc::mi_free(p) };
}

unsafe extern "C" fn test_visit(
    _heap: *const mi_heap_t,
    area: *const mi_heap_area_t,
    block: *mut c_void,
    block_size: usize,
    arg: *mut c_void,
) -> bool {
    let state = unsafe { &mut *arg.cast::<VisitState>() };
    let area = unsafe { &*area };
    if block.is_null() {
        state.areas += 1;
        assert!(area.full_block_size >= area.block_size);
    } else {
        state.blocks += 1;
        let _ = block_size;
        let _ = unsafe { mimalloc::mi_usable_size(block) };
    }
    true
}

unsafe fn test_heap_walk() {
    let heap = unsafe { mimalloc::mi_heap_new() };
    assert!(!heap.is_null());

    let _p1 = unsafe { mimalloc::mi_heap_malloc(heap, 16 * 2_097_152) };
    let _p2 = unsafe { mimalloc::mi_heap_malloc(heap, 2_067_152) };
    let _p3 = unsafe { mimalloc::mi_heap_malloc(heap, 2_097_160) };
    let _p4 = unsafe { mimalloc::mi_heap_malloc(heap, 24_576) };

    let mut state = VisitState::default();
    let ok = unsafe {
        mimalloc::mi_heap_visit_blocks(
            heap,
            true,
            Some(test_visit),
            (&mut state as *mut VisitState).cast(),
        )
    };
    assert!(ok);
    assert!(state.areas > 0);
    assert!(state.blocks > 0);

    unsafe { mimalloc::mi_heap_delete(heap) };
}

#[cfg(windows)]
unsafe fn test_manage_os_memory() {
    let _manage: unsafe extern "C" fn(
        *mut c_void,
        usize,
        bool,
        bool,
        bool,
        i32,
        bool,
        *mut mi_arena_id_t,
    ) -> bool = mimalloc::mi_manage_os_memory_ex;

    let heap = unsafe { mimalloc::mi_heap_new_in_arena(ptr::null_mut()) };
    assert!(!heap.is_null());

    let p1 = unsafe { mimalloc::mi_heap_malloc(heap, 8) };
    let p2 = unsafe { mimalloc::mi_heap_malloc(heap, core::mem::size_of::<i32>()) }.cast::<i32>();
    assert!(!p2.is_null());
    unsafe { *p2 = 42 };

    let prev_default_theap =
        unsafe { mimalloc::mi_theap_set_default(mimalloc::mi_heap_theap(heap)) };
    let p3 = unsafe { mimalloc::mi_malloc(8) };
    unsafe { mimalloc::mi_free(p3) };
    unsafe { mimalloc::mi_theap_set_default(prev_default_theap) };

    unsafe { mimalloc::mi_free(p1) };
    unsafe { mimalloc::mi_free(p2.cast()) };
    unsafe { mimalloc::mi_heap_delete(heap) };
}

#[cfg(not(windows))]
unsafe fn test_manage_os_memory() {}

unsafe fn test_large_pages() {
    let mut arena_id: mi_arena_id_t = ptr::null_mut();
    let err = unsafe { mimalloc::mi_reserve_huge_os_pages_at_ex(0, -1, 0, false, &mut arena_id) };
    assert_eq!(err, 0);
    assert!(arena_id.is_null());
}

fn _mi_bin4(size: usize) -> u8 {
    let wsize = _mi_wsize_from_size(size);
    if wsize <= 1 {
        return 1;
    }
    if cfg!(target_pointer_width = "32") && wsize <= 4 {
        return ((wsize + 1) & !1) as u8;
    }
    if cfg!(target_pointer_width = "64") && wsize <= 8 {
        return ((wsize + 1) & !1) as u8;
    }
    if !cfg!(any(
        target_pointer_width = "32",
        target_pointer_width = "64"
    )) && wsize <= 8
    {
        return wsize as u8;
    }
    if wsize > LARGE_WSIZE_MAX {
        return MI_BIN_HUGE as u8;
    }

    let b = (MI_SIZE_BITS - 1 - mi_clz(wsize)) as u8;
    ((b << 1) + ((wsize >> (b - 1)) & 0x01) as u8) + 3
}

fn mi_bins() {
    let mut last_bin = 0usize;
    let mut transitions = 0usize;
    for wsize in 1..=((4 * 1024 * 1024) / 8 + 1024) {
        let size = wsize * MI_INTPTR_SIZE;
        let bin = page::mi_bin(size);
        let alt_bin = _mi_bin4(size);
        assert!(alt_bin <= MI_BIN_HUGE as u8);
        if bin != last_bin {
            transitions += 1;
            last_bin = bin;
        }
    }
    assert!(transitions > 0);
}

unsafe fn test_canary_leak() {
    let p = unsafe { mimalloc::mi_mallocn(22, core::mem::size_of::<u8>()) }.cast::<u8>();
    assert!(!p.is_null());
    for i in 0..22usize {
        unsafe { *p.add(i) = b'0'.wrapping_add(i as u8) };
    }
    assert!(unsafe { mimalloc::mi_usable_size(p.cast()) } >= 22);
    unsafe { mimalloc::mi_free(p.cast()) };
}

fn test_dep() {}

unsafe fn main_override_static_body() {
    assert!(unsafe { mimalloc::mi_version() } > 0);
    unsafe { mimalloc::mi_stats_reset() };

    test_dep();
    unsafe { test_canary_leak() };

    let mut p1 = unsafe { mimalloc::mi_malloc(78) };
    let mut p2 = unsafe { mimalloc::mi_malloc(24) };
    unsafe { mimalloc::mi_free(p1) };
    p1 = unsafe { mimalloc::mi_malloc(8) };

    let hello = CString::new("hello\n").expect("static string must be valid C string");
    let s = unsafe { mimalloc::mi_strdup(hello.as_ptr()) };

    unsafe { mimalloc::mi_free(p2) };
    p2 = unsafe { mimalloc::mi_malloc(16) };
    p1 = unsafe { mimalloc::mi_realloc(p1, 32) };
    unsafe { mimalloc::mi_free(p1) };
    unsafe { mimalloc::mi_free(p2) };
    unsafe { mimalloc::mi_free(s.cast()) };
}

#[test]
fn main_override_static_characterization() {
    unsafe { main_override_static_body() };
}

#[test]
fn alloc_huge_smoke() {
    unsafe { alloc_huge() };
}

#[test]
fn heap_walk_smoke() {
    unsafe { test_heap_walk() };
}

#[test]
fn manage_os_memory_characterization() {
    unsafe { test_manage_os_memory() };
}

#[test]
fn large_pages_characterization() {
    unsafe { test_large_pages() };
}

#[test]
fn bins_characterization() {
    mi_bins();
}
