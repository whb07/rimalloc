#![cfg(all(unix, feature = "std", not(feature = "c-abi")))]

use core::ffi::c_void;
use core::ptr;

use mechanical_mimalloc::raw::{MI_STAT_VERSION, mi_process_init, mi_stats_get, mi_stats_t};

unsafe extern "C" {
    fn mi_os_alloc_aligned(
        size: usize,
        alignment: usize,
        commit: bool,
        base: *mut *mut c_void,
        full_size: *mut usize,
    ) -> *mut c_void;
    fn mi_os_free(p: *mut c_void, size: usize);
}

fn initialized_stats() -> mi_stats_t {
    mi_stats_t {
        size: core::mem::size_of::<mi_stats_t>(),
        version: MI_STAT_VERSION,
        ..Default::default()
    }
}

unsafe fn allocate_aligned(size: usize, alignment: usize) -> *mut c_void {
    let mut base = ptr::null_mut();
    let mut full_size = 0;
    // SAFETY: the out-pointers are valid and writable, and `size` and `alignment`
    // are nonzero page multiples with a power-of-two alignment.
    let p = unsafe { mi_os_alloc_aligned(size, alignment, true, &mut base, &mut full_size) };
    assert!(!p.is_null());
    assert!(!base.is_null());
    assert_eq!(p, base);
    assert_eq!(full_size, size);
    assert_eq!((p as usize) % alignment, 0);
    p
}

#[test]
fn committed_aligned_os_trim_balances_reserved_and_committed_stats() {
    // SAFETY: this test runs in its own integration-test process and initializes
    // mimalloc before calling its low-level exported OS allocation functions.
    unsafe { mi_process_init() };

    // SAFETY: `_SC_PAGESIZE` has no pointer arguments or side effects beyond
    // querying the initialized host OS configuration.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    assert!(page_size > 0);
    let size = usize::try_from(page_size).unwrap();
    let alignment = size.checked_mul(256).unwrap();
    assert!(alignment.is_power_of_two());

    // Warm the process-global OS configuration and allocation path before the
    // baseline snapshot.
    // SAFETY: `size` and `alignment` satisfy `allocate_aligned`'s contract.
    let warm = unsafe { allocate_aligned(size, alignment) };
    // SAFETY: `warm..warm+size` is the complete live range returned above.
    unsafe { mi_os_free(warm, size) };

    let mut baseline = initialized_stats();
    assert!(unsafe { mi_stats_get(&mut baseline) });

    // SAFETY: `size` and `alignment` satisfy `allocate_aligned`'s contract.
    let p = unsafe { allocate_aligned(size, alignment) };
    let mut live = initialized_stats();
    assert!(unsafe { mi_stats_get(&mut live) });
    assert_eq!(
        live.reserved.current - baseline.reserved.current,
        size as i64
    );
    assert_eq!(
        live.committed.current - baseline.committed.current,
        size as i64
    );

    // SAFETY: `p..p+size` is the complete live range returned above.
    unsafe { mi_os_free(p, size) };
    let mut after = initialized_stats();
    assert!(unsafe { mi_stats_get(&mut after) });
    assert_eq!(after.reserved.current, baseline.reserved.current);
    assert_eq!(after.committed.current, baseline.committed.current);
}
