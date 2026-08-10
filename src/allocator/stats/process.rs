//! Process timing and operating-system information collection.

use core::sync::atomic::{AtomicI64, Ordering};

use crate::init::_mi_subproc_main;
use crate::prim::{_mi_prim_clock_now, _mi_prim_process_info};
use crate::types::mi_process_info_t;

use super::MiSubprocStatsPrefix;

static CLOCK_DIFF: AtomicI64 = AtomicI64::new(0);
static PROCESS_START: AtomicI64 = AtomicI64::new(0);

fn clock_now_msecs() -> i64 {
    // SAFETY: stats clock helpers delegate to primitive monotonic clock reads without allocation.
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

#[used]
static MI_STATS_INIT_KEEP: extern "C" fn() = _mi_stats_init;

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

#[inline(always)]
pub(super) unsafe fn mi_process_info_impl(output: ProcessInfoOutput) {
    let mut pinfo = mi_process_info_t {
        elapsed: _mi_clock_end(PROCESS_START.load(Ordering::Relaxed)),
        ..Default::default()
    };
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    let subproc = unsafe { _mi_subproc_main() }.cast::<MiSubprocStatsPrefix>();
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    if let Some(subproc) = unsafe { subproc.as_ref() } {
        pinfo.current_commit = subproc.stats.committed.current.max(0) as usize;
        pinfo.peak_commit = subproc.stats.committed.peak.max(0) as usize;
    }
    pinfo.current_rss = pinfo.current_commit;
    pinfo.peak_rss = pinfo.peak_commit;
    // SAFETY: `pinfo` is a valid mutable primitive info struct.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        _mi_prim_process_info(&mut pinfo);
        if !output.elapsed_msecs.is_null() {
            *output.elapsed_msecs = clamp_msecs_to_size(pinfo.elapsed);
        }
        if !output.user_msecs.is_null() {
            *output.user_msecs = clamp_msecs_to_size(pinfo.utime);
        }
        if !output.system_msecs.is_null() {
            *output.system_msecs = clamp_msecs_to_size(pinfo.stime);
        }
        if !output.current_rss.is_null() {
            *output.current_rss = pinfo.current_rss;
        }
        if !output.peak_rss.is_null() {
            *output.peak_rss = pinfo.peak_rss;
        }
        if !output.current_commit.is_null() {
            *output.current_commit = pinfo.current_commit;
        }
        if !output.peak_commit.is_null() {
            *output.peak_commit = pinfo.peak_commit;
        }
        if !output.page_faults.is_null() {
            *output.page_faults = pinfo.page_faults;
        }
    }
}
