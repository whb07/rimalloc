//! Process timing and operating-system information collection.

use core::sync::atomic::{AtomicI64, Ordering};

use crate::allocator::init::_mi_subproc_main;
use crate::platform::{_mi_prim_clock_now, _mi_prim_process_info};
use crate::types::mi_process_info_t;

use super::MiSubprocStatsPrefix;

static CLOCK_DIFF: AtomicI64 = AtomicI64::new(0);
static PROCESS_START: AtomicI64 = AtomicI64::new(0);

fn clock_now_msecs() -> i64 {
    unsafe { _mi_prim_clock_now() }
}

#[inline(never)]
pub(crate) extern "C" fn _mi_clock_now() -> i64 {
    clock_now_msecs()
}

#[inline(never)]
pub(crate) extern "C" fn _mi_clock_start() -> i64 {
    let current = CLOCK_DIFF.load(Ordering::Relaxed);
    if current == 0 {
        let t0 = _mi_clock_now();
        let diff = _mi_clock_now() - t0;
        let _ = CLOCK_DIFF.compare_exchange(0, diff, Ordering::Relaxed, Ordering::Relaxed);
    }
    _mi_clock_now()
}

#[inline(never)]
pub(crate) extern "C" fn _mi_clock_end(start: i64) -> i64 {
    _mi_clock_now() - start - CLOCK_DIFF.load(Ordering::Relaxed)
}

#[inline(never)]
pub(crate) extern "C" fn _mi_stats_init() {
    if PROCESS_START.load(Ordering::Relaxed) == 0 {
        let start = _mi_clock_start();
        let _ = PROCESS_START.compare_exchange(0, start, Ordering::Relaxed, Ordering::Relaxed);
    }
}

fn clamp_msecs_to_size(value: i64) -> usize {
    if value <= 0 {
        0
    } else {
        value.min(isize::MAX as i64) as usize
    }
}

pub(super) struct ProcessInfoOutput {
    pub(super) elapsed_msecs: *mut usize,
    pub(super) user_msecs: *mut usize,
    pub(super) system_msecs: *mut usize,
    pub(super) current_rss: *mut usize,
    pub(super) peak_rss: *mut usize,
    pub(super) current_commit: *mut usize,
    pub(super) peak_commit: *mut usize,
    pub(super) page_faults: *mut usize,
}

/// # Safety
/// - Pointer validity: each non-null pointer in `output` must be valid for writing
///   one `usize`, properly aligned for `usize`, and dereferenceable for a single
///   `usize` write.
/// - Lifetime: each non-null output pointer must remain live and not be freed,
///   reallocated, or invalidated for the entire call.
/// - Aliasing: output destinations must not alias each other or the internal
///   `mi_process_info_t` in a way that creates overlapping mutable access; the
///   caller must provide exclusive write access to each non-null destination.
/// - Initialization: `pinfo` is initialized on the stack with `Default` and
///   `elapsed`; output destinations are written only after `pinfo` is fully
///   populated by the platform primitive.
/// - Ownership: caller retains ownership of all output allocations; this function
///   does not allocate, free, or transfer ownership.
/// - Threading: caller must synchronize concurrent writes to the same output
///   storage; internal subproc snapshot uses relaxed atomics and tolerates
///   concurrent stats updates; the platform primitive is thread-safe.
/// - Unwind: this function does not unwind; the platform primitive must not
///   unwind across the FFI boundary.
#[inline(always)]
pub(super) unsafe fn mi_process_info_impl(output: ProcessInfoOutput) {
    let mut pinfo = mi_process_info_t {
        elapsed: _mi_clock_end(PROCESS_START.load(Ordering::Relaxed)),
        ..Default::default()
    };
    let subproc = _mi_subproc_main().cast::<MiSubprocStatsPrefix>();
    if let Some(subproc) = unsafe { subproc.as_ref() } {
        pinfo.current_commit = subproc.stats.committed.current.max(0) as usize;
        pinfo.peak_commit = subproc.stats.committed.peak.max(0) as usize;
    }
    pinfo.current_rss = pinfo.current_commit;
    pinfo.peak_rss = pinfo.peak_commit;
    unsafe {
        _mi_prim_process_info(&mut pinfo);
    }
    if !output.elapsed_msecs.is_null() {
        let value = clamp_msecs_to_size(pinfo.elapsed);
        unsafe {
            *output.elapsed_msecs = value;
        }
    }
    if !output.user_msecs.is_null() {
        let value = clamp_msecs_to_size(pinfo.utime);
        unsafe {
            *output.user_msecs = value;
        }
    }
    if !output.system_msecs.is_null() {
        let value = clamp_msecs_to_size(pinfo.stime);
        unsafe {
            *output.system_msecs = value;
        }
    }
    if !output.current_rss.is_null() {
        let value = pinfo.current_rss;
        unsafe {
            *output.current_rss = value;
        }
    }
    if !output.peak_rss.is_null() {
        let value = pinfo.peak_rss;
        unsafe {
            *output.peak_rss = value;
        }
    }
    if !output.current_commit.is_null() {
        let value = pinfo.current_commit;
        unsafe {
            *output.current_commit = value;
        }
    }
    if !output.peak_commit.is_null() {
        let value = pinfo.peak_commit;
        unsafe {
            *output.peak_commit = value;
        }
    }
    if !output.page_faults.is_null() {
        let value = pinfo.page_faults;
        unsafe {
            *output.page_faults = value;
        }
    }
}
