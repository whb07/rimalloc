use core::ffi::c_void;
use core::ptr;
use std::time::{Duration, Instant};

use rimalloc::raw::*;

fn initialized_stats() -> mi_stats_t {
    mi_stats_t {
        size: core::mem::size_of::<mi_stats_t>(),
        version: MI_STAT_VERSION,
        ..Default::default()
    }
}

#[test]
fn lifecycle_theap_and_stats_exports_link_and_smoke() {
    unsafe {
        mi_process_init();
        let process_done: unsafe extern "C" fn() = mi_process_done;
        core::hint::black_box(process_done);
        assert!(!mi_is_redirected());

        let mut init_stats = initialized_stats();
        assert!(mi_stats_get(&mut init_stats));
        assert_eq!(init_stats.heaps.current, 1);
        assert_eq!(init_stats.heaps.peak, 1);
        assert_eq!(init_stats.heaps.total, 1);

        let default_theap = mi_theap_get_default();
        assert!(!default_theap.is_null());
        assert_eq!(mi_theap_set_default(default_theap), default_theap);

        let routed_heap = mi_heap_new();
        assert!(!routed_heap.is_null());
        let routed_theap = mi_heap_theap(routed_heap);
        assert!(!routed_theap.is_null());
        assert_ne!(routed_theap, default_theap);
        assert_eq!(mi_theap_set_default(routed_theap), default_theap);
        let routed = mi_malloc(48);
        assert!(!routed.is_null());
        assert!(mi_heap_contains(routed_heap, routed));
        assert!(mi_any_heap_contains(routed));
        assert!(!mi_heap_contains(ptr::null(), routed));
        assert!(!mi_unsafe_heap_page_is_under_utilized(
            routed_heap,
            ptr::null_mut(),
            100
        ));
        let under_utilized = mi_unsafe_heap_page_is_under_utilized(routed_heap, routed, 100);
        core::hint::black_box(under_utilized);
        mi_free(routed);
        assert_eq!(mi_theap_set_default(default_theap), routed_theap);
        mi_heap_delete(routed_heap);

        let p = mi_theap_malloc_small(default_theap, 32);
        assert!(!p.is_null());
        assert!(mi_usable_size(p) >= 32);
        mi_free(p);

        assert!(mi_arena_min_alignment().is_power_of_two());

        let mut stats = initialized_stats();
        assert!(mi_stats_get(&mut stats));

        let heap = mi_heap_new();
        assert!(!heap.is_null());
        let p = mi_heap_malloc(heap, 64);
        assert!(!p.is_null());

        let mut heap_stats = initialized_stats();
        assert!(mi_heap_stats_get(heap, &mut heap_stats));
        mi_heap_stats_merge_to_subproc(heap);

        mi_free(p.cast::<c_void>());
        mi_heap_delete(heap);

        mi_stats_merge();
        mi_collect_reduce(0);

        let mut arena_id = ptr::null_mut();
        assert!(!mi_manage_memory(
            ptr::null_mut(),
            0,
            false,
            false,
            true,
            -1,
            false,
            None,
            ptr::null_mut(),
            &mut arena_id,
        ));
        assert!(arena_id.is_null());
    }
}

#[test]
fn process_info_reports_cpu_time_after_work() {
    unsafe {
        mi_process_init();

        let start = Instant::now();
        let mut n = 0usize;
        while start.elapsed() < Duration::from_millis(30) {
            n = n.wrapping_add(1);
            core::hint::black_box(n);
        }

        let mut elapsed = 0usize;
        let mut user = 0usize;
        let mut system = 0usize;
        mi_process_info(
            &mut elapsed,
            &mut user,
            &mut system,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        );

        assert!(elapsed > 0);
        assert!(user + system > 0);
    }
}
