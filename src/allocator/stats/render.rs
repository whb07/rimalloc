//! Bounded text and JSON statistics rendering.

use core::ffi::{CStr, c_char, c_void};
use core::{mem::size_of, ptr, str};

use crate::allocator::options::{_mi_fprintf, _mi_fputs};
use crate::allocator::page::{
    MI_LARGE_MAX_OBJ_SIZE, MI_LARGE_PAGE_SIZE, MI_MEDIUM_MAX_OBJ_SIZE, MI_MEDIUM_PAGE_SIZE,
    MI_SMALL_MAX_OBJ_SIZE, MI_SMALL_PAGE_SIZE, mi_good_size_typed,
};
use crate::ffi::libc;
use crate::ffi::mimalloc::{mi_free, mi_rezalloc};
use crate::platform::os::_mi_os_numa_node_count;
use crate::types::{MI_MALLOC_VERSION, mi_output_fun};
use crate::types::{RequestedSize, StatsBin};

use super::counters::{mi_stats_get_bin_size_typed, mi_stats_snapshot_atomic, stats_bin_iter};
use super::process::{ProcessInfoOutput, mi_process_info_impl};
use super::{
    MI_BIN_HUGE, MI_CHUNKBIN_COUNT, MI_CHUNKBINS, MI_STAT_VERSION, mi_chunkbin_e, mi_chunkbin_t,
    mi_stat_count_t, mi_stat_counter_t, mi_stats_t,
};

#[repr(C)]
struct buffered_s {
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    buf: *mut c_char,
    used: usize,
    count: usize,
}

type buffered_t = buffered_s;

#[derive(Clone, Copy)]
struct StatCountValue(i64);

impl StatCountValue {
    #[inline(always)]
    fn get(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy)]
struct StatCountSnapshot {
    total: StatCountValue,
    peak: StatCountValue,
    current: StatCountValue,
}

impl StatCountSnapshot {
    #[inline(always)]
    fn from_stat(stat: &mi_stat_count_t) -> Self {
        Self {
            total: StatCountValue(stat.total),
            peak: StatCountValue(stat.peak),
            current: StatCountValue(stat.current),
        }
    }
}

#[derive(Clone, Copy)]
struct StatCounterSnapshot {
    total: StatCountValue,
}

impl StatCounterSnapshot {
    #[inline(always)]
    fn from_stat(stat: &mi_stat_counter_t) -> Self {
        Self {
            total: StatCountValue(stat.total),
        }
    }
}

fn c_char_buffer_str(buf: &[c_char]) -> &str {
    // SAFETY: fixed formatting buffers are written only from ASCII format fragments and
    // always NUL-terminated by `mi_vsnprintf_args`.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe { str::from_utf8_unchecked(CStr::from_ptr(buf.as_ptr()).to_bytes()) }
}

fn format_amount(buf: &mut [c_char; 32], mut n: i64, unit: i64) {
    let suffix = if unit <= 0 { " " } else { "B" };
    let base = if unit == 0 { 1000 } else { 1024 };
    if unit > 0 {
        n = n.saturating_mul(unit);
    }

    let pos = if n < 0 { n.saturating_neg() } else { n };
    if pos < base {
        if n != 1 || suffix != "B" {
            let amount_suffix = if n == 0 { "" } else { suffix };
            libc::mi_vsnprintf_args(buf, format_args!("{n}   {amount_suffix:<3}"));
        } else {
            buf[0] = 0;
        }
    } else {
        let mut divider = base;
        let mut magnitude = "K";
        if pos >= divider * base {
            divider *= base;
            magnitude = "M";
        }
        if pos >= divider * base {
            divider *= base;
            magnitude = "G";
        }
        let tens = n / (divider / 10);
        let whole = tens / 10;
        let frac1 = (tens % 10).abs();
        let unitdesc = if base == 1024 {
            match magnitude {
                "K" => "KiB",
                "M" => "MiB",
                _ => "GiB",
            }
        } else {
            match magnitude {
                "K" => "K ",
                "M" => "M ",
                _ => "G ",
            }
        };
        libc::mi_vsnprintf_args(buf, format_args!("{whole}.{frac1} {unitdesc:<3}"));
    }
}

fn mi_printf_amount(
    n: i64,
    unit: i64,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    fmt: *const c_char,
) {
    let mut buf = [0 as c_char; 32];
    format_amount(&mut buf, n, unit);
    let rendered = c_char_buffer_str(&buf);
    if fmt.is_null() {
        _mi_fprintf(out, arg, format_args!("{rendered:>12}"));
        return;
    }
    // SAFETY: `fmt` is expected to point to a valid NUL-terminated C string.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    let fmt_bytes = unsafe { CStr::from_ptr(fmt) }.to_bytes();
    if fmt_bytes == b"%s" {
        _mi_fprintf(out, arg, format_args!("{rendered}"));
    } else {
        _mi_fprintf(out, arg, format_args!("{rendered:>12}"));
    }
}

fn mi_print_amount(n: i64, unit: i64, out: Option<mi_output_fun>, arg: *mut c_void) {
    mi_printf_amount(n, unit, out, arg, ptr::null());
}

fn mi_print_count(n: i64, unit: i64, out: Option<mi_output_fun>, arg: *mut c_void) {
    if unit == 1 {
        _mi_fprintf(out, arg, format_args!("{:>12}", " "));
    } else {
        mi_print_amount(n, 0, out, arg);
    }
}

fn mi_stat_print_ex(
    stat: &mi_stat_count_t,
    msg: &str,
    unit: i64,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    notok: Option<&str>,
) {
    let stat = StatCountSnapshot::from_stat(stat);
    _mi_fprintf(out, arg, format_args!("  {msg:<10}:"));
    if unit != 0 {
        if unit > 0 {
            mi_print_amount(stat.peak.get(), unit, out, arg);
            mi_print_amount(stat.total.get(), unit, out, arg);
            mi_print_amount(stat.current.get(), unit, out, arg);
            mi_print_amount(unit, 1, out, arg);
            mi_print_count(stat.total.get(), unit, out, arg);
        } else {
            mi_print_amount(stat.peak.get(), -1, out, arg);
            mi_print_amount(stat.total.get(), -1, out, arg);
            mi_print_amount(stat.current.get(), -1, out, arg);
            if unit == -1 {
                _mi_fprintf(out, arg, format_args!("{:>24}", ""));
            } else {
                mi_print_amount(-unit, 1, out, arg);
                mi_print_count(stat.total.get() / -unit, 0, out, arg);
            }
        }
        if stat.current.get() != 0 {
            _mi_fprintf(
                out,
                arg,
                format_args!("  {}\n", notok.unwrap_or("not all freed")),
            );
        } else {
            _mi_fprintf(out, arg, format_args!("  ok\n"));
        }
    } else {
        mi_print_amount(stat.peak.get(), 0, out, arg);
        mi_print_amount(stat.total.get(), 0, out, arg);
        mi_print_amount(stat.current.get(), 0, out, arg);
        _mi_fprintf(out, arg, format_args!("\n"));
    }
}

fn mi_stat_print(
    stat: &mi_stat_count_t,
    msg: &str,
    unit: i64,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    mi_stat_print_ex(stat, msg, unit, out, arg, None);
}

fn mi_stat_total_print(
    stat: &mi_stat_count_t,
    msg: &str,
    unit: i64,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let stat = StatCountSnapshot::from_stat(stat);
    _mi_fprintf(out, arg, format_args!("  {msg:<10}:"));
    _mi_fprintf(out, arg, format_args!("{:>12}", " "));
    mi_print_amount(stat.total.get(), unit, out, arg);
    _mi_fprintf(out, arg, format_args!("\n"));
}

fn mi_stat_counter_print(
    stat: &mi_stat_counter_t,
    msg: &str,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let stat = StatCounterSnapshot::from_stat(stat);
    _mi_fprintf(out, arg, format_args!("  {msg:<10}:"));
    mi_print_amount(stat.total.get(), 0, out, arg);
    _mi_fprintf(out, arg, format_args!("\n"));
}

fn mi_stat_counter_print_size(
    stat: &mi_stat_counter_t,
    msg: &str,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let stat = StatCounterSnapshot::from_stat(stat);
    _mi_fprintf(out, arg, format_args!("  {msg:<10}:"));
    mi_print_amount(stat.total.get(), 1, out, arg);
    _mi_fprintf(out, arg, format_args!("\n"));
}

fn mi_stat_average_print(
    count: i64,
    total: i64,
    msg: &str,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) {
    let avg_tens = if count == 0 { 0 } else { total * 10 / count };
    let avg_whole = avg_tens / 10;
    let avg_frac1 = avg_tens % 10;
    _mi_fprintf(
        out,
        arg,
        format_args!("  {msg:<10}: {avg_whole:>5}.{avg_frac1} avg\n"),
    );
}

fn mi_print_header(name: &str, out: Option<mi_output_fun>, arg: *mut c_void) {
    _mi_fprintf(
        out,
        arg,
        format_args!(
            " {name:<11} {:>11} {:>11} {:>11} {:>11} {:>11}\n",
            "peak   ", "total   ", "current   ", "block   ", "total#   "
        ),
    );
}

fn mi_stats_print_bins(
    bins: &[mi_stat_count_t],
    max: StatsBin,
    out: Option<mi_output_fun>,
    arg: *mut c_void,
) -> bool {
    let mut found = false;
    for bin in stats_bin_iter().take(max.as_usize() + 1) {
        let stat = &bins[bin.as_usize()];
        if StatCountSnapshot::from_stat(stat).total.get() > 0 {
            found = true;
            let unit = mi_stats_get_bin_size_typed(bin);
            let pagekind = if unit.get() <= MI_SMALL_MAX_OBJ_SIZE {
                "S"
            } else if unit.get() <= MI_MEDIUM_MAX_OBJ_SIZE {
                "M"
            } else if unit.get() <= MI_LARGE_MAX_OBJ_SIZE {
                "L"
            } else {
                "H"
            };
            let mut label = [0 as c_char; 64];
            libc::mi_vsnprintf_args(
                &mut label,
                format_args!("bin{pagekind:>2}  {:>3}", bin.as_usize()),
            );
            mi_stat_print(stat, c_char_buffer_str(&label), unit.as_bytes(), out, arg);
        }
    }
    if found {
        _mi_fprintf(out, arg, format_args!("\n"));
    }
    found
}

fn mi_buffered_flush(buf: &mut buffered_t) {
    if buf.buf.is_null() {
        return;
    }
    // SAFETY: `buf.buf` points to writable storage of at least `count + 1` bytes.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        *buf.buf.add(buf.used) = 0;
        _mi_fputs(buf.out, buf.arg, ptr::null(), buf.buf);
    }
    buf.used = 0;
}

/// # Safety
/// A non-null `msg` must point to a readable NUL-terminated string. A non-null `arg` must point to
/// the live, aligned, uniquely writable `buffered_t` created by `_mi_stats_print`, including its
/// writable backing buffer. This C callback must not unwind.
unsafe extern "C" fn mi_buffered_out(msg: *const c_char, arg: *mut c_void) {
    if msg.is_null() || arg.is_null() {
        return;
    }
    // SAFETY: caller passes a valid `buffered_t` as `arg` and `msg` is a NUL-terminated string.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        let buf = &mut *(arg as *mut buffered_t);
        let mut src = msg;
        while *src != 0 {
            let c = *src;
            if buf.used >= buf.count {
                mi_buffered_flush(buf);
            }
            *buf.buf.add(buf.used) = c;
            buf.used += 1;
            if c == b'\n' as c_char {
                mi_buffered_flush(buf);
            }
            src = src.add(1);
        }
    }
}

/// # Safety
/// `stats` must be null or point to live, aligned initialized stats. During the snapshot, each
/// source counter must either be exclusively stable or have all concurrent access use the atomic
/// stats protocol. A non-null output callback and `arg0` must remain valid for synchronous
/// invocation and the callback must not unwind.
pub(super) unsafe fn _mi_stats_print(
    name: &str,
    id: usize,
    stats: *const mi_stats_t,
    out0: Option<mi_output_fun>,
    arg0: *mut c_void,
) {
    if stats.is_null() {
        return;
    }
    // SAFETY: the caller keeps the non-null source live and each counter is either exclusively
    // stable or accessed concurrently only through the atomic stats protocol.
    let snapshot = unsafe { mi_stats_snapshot_atomic(stats) };
    let stats = &snapshot;

    let mut buf = [0 as c_char; 256];
    let mut buffer = buffered_t {
        out: out0,
        arg: arg0,
        buf: ptr::null_mut(),
        used: 0,
        count: 255,
    };
    buffer.buf = buf.as_mut_ptr();
    let out: Option<mi_output_fun> = Some(mi_buffered_out as mi_output_fun);
    let arg = (&mut buffer as *mut buffered_t).cast::<c_void>();

    _mi_fprintf(out, arg, format_args!("{name} {id}\n"));

    if stats.malloc_normal.total + stats.malloc_huge.total != 0 {
        mi_print_header("blocks", out, arg);
        mi_stats_print_bins(
            &stats.malloc_bins,
            // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
            unsafe { StatsBin::new_unchecked(MI_BIN_HUGE as u16) },
            out,
            arg,
        );
        mi_stat_print(
            &stats.malloc_normal,
            "binned",
            if stats.malloc_normal_count.total == 0 {
                -1
            } else {
                1
            },
            out,
            arg,
        );
        mi_stat_print(
            &stats.malloc_huge,
            "huge",
            if stats.malloc_huge_count.total == 0 {
                -1
            } else {
                1
            },
            out,
            arg,
        );
        let mut total = mi_stat_count_t {
            current: stats.malloc_normal.current + stats.malloc_huge.current,
            total: stats.malloc_normal.total + stats.malloc_huge.total,
            peak: stats.malloc_normal.peak,
        };
        total.peak = total
            .peak
            .max(stats.malloc_normal.current + stats.malloc_huge.peak);
        mi_stat_print_ex(&total, "total", 1, out, arg, Some(""));
        mi_stat_total_print(&stats.malloc_requested, "malloc req", 1, out, arg);
        _mi_fprintf(out, arg, format_args!("\n"));
    }

    if stats.pages.total != 0 {
        mi_print_header("pages", out, arg);
        mi_stat_print_ex(&stats.page_committed, "touched", 1, out, arg, Some(""));
        mi_stat_print(&stats.pages, "pages", 0, out, arg);
        mi_stat_print(&stats.pages_abandoned, "abandoned", 0, out, arg);
        mi_stat_counter_print(&stats.pages_reclaim_on_alloc, "reclaima", out, arg);
        mi_stat_counter_print(&stats.pages_reclaim_on_free, "reclaimf", out, arg);
        mi_stat_counter_print(&stats.pages_reabandon_full, "reabandon", out, arg);
        mi_stat_counter_print(&stats.pages_unabandon_busy_wait, "waits", out, arg);
        mi_stat_counter_print(&stats.pages_extended, "extended", out, arg);
        mi_stat_counter_print(&stats.pages_retire, "retire", out, arg);
        mi_stat_average_print(
            stats.page_searches_count.total,
            stats.page_searches.total,
            "searches",
            out,
            arg,
        );
        _mi_fprintf(out, arg, format_args!("\n"));
    }

    if stats.arena_count.total > 0 {
        mi_print_header("arenas", out, arg);
        mi_stat_print_ex(&stats.reserved, "reserved", 1, out, arg, Some(""));
        mi_stat_print_ex(&stats.committed, "committed", 1, out, arg, Some(""));
        mi_stat_counter_print_size(&stats.reset, "reset", out, arg);
        mi_stat_counter_print_size(&stats.purged, "purged", out, arg);
        mi_stat_counter_print(&stats.arena_count, "arenas", out, arg);
        mi_stat_counter_print(&stats.arena_rollback_count, "rollback", out, arg);
        mi_stat_counter_print(&stats.mmap_calls, "mmaps", out, arg);
        mi_stat_counter_print(&stats.commit_calls, "commits", out, arg);
        mi_stat_counter_print(&stats.reset_calls, "resets", out, arg);
        mi_stat_counter_print(&stats.purge_calls, "purges", out, arg);
        mi_stat_counter_print(&stats.malloc_guarded_count, "guarded", out, arg);
        mi_stat_print_ex(&stats.theaps, "theaps", 0, out, arg, Some(""));
        mi_stat_print_ex(&stats.heaps, "heaps", 0, out, arg, Some(""));
        mi_stat_counter_print(&stats.heaps_delete_wait, "heap waits", out, arg);
        _mi_fprintf(out, arg, format_args!("\n"));

        mi_print_header("process", out, arg);
        mi_stat_print_ex(&stats.threads, "threads", 0, out, arg, Some(""));
        _mi_fprintf(
            out,
            arg,
            format_args!("  {:<10}: {:>5}\n", "numa nodes", _mi_os_numa_node_count()),
        );
        // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
        unsafe { mi_process_info_print_out_impl(out, arg) };
    }
    _mi_fprintf(out, arg, format_args!("\n"));
}

#[inline(always)]
/// # Safety
/// A non-null output callback and `arg` must remain valid for synchronous invocation and the
/// callback must not unwind.
pub(super) unsafe fn mi_process_info_print_out_impl(out: Option<mi_output_fun>, arg: *mut c_void) {
    let mut elapsed = 0usize;
    let mut user_time = 0usize;
    let mut sys_time = 0usize;
    let mut current_rss = 0usize;
    let mut peak_rss = 0usize;
    let mut current_commit = 0usize;
    let mut peak_commit = 0usize;
    let mut page_faults = 0usize;
    // SAFETY: local pointers below are valid writable outputs.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        mi_process_info_impl(ProcessInfoOutput {
            elapsed_msecs: &mut elapsed,
            user_msecs: &mut user_time,
            system_msecs: &mut sys_time,
            current_rss: &mut current_rss,
            peak_rss: &mut peak_rss,
            current_commit: &mut current_commit,
            peak_commit: &mut peak_commit,
            page_faults: &mut page_faults,
        });
    }
    _mi_fprintf(
        out,
        arg,
        format_args!(
            "  {:<10}: {:>5}.{:03} s\n",
            "elapsed",
            elapsed / 1000,
            elapsed % 1000
        ),
    );
    _mi_fprintf(
        out,
        arg,
        format_args!(
            "  {:<10}: user: {}.{:03} s, system: {}.{:03} s, faults: {}, peak rss: ",
            "process",
            user_time / 1000,
            user_time % 1000,
            sys_time / 1000,
            sys_time % 1000,
            page_faults
        ),
    );
    mi_printf_amount(peak_rss as i64, 1, out, arg, c"%s".as_ptr());
    if peak_commit > 0 {
        _mi_fprintf(out, arg, format_args!(", peak commit: "));
        mi_printf_amount(peak_commit as i64, 1, out, arg, c"%s".as_ptr());
    }
    _mi_fprintf(out, arg, format_args!("\n"));
}

#[inline(always)]
pub(super) fn mi_process_info_print_impl() {
    // Keep the public-symbol call so interposition and release codegen match the pre-split path.
    // SAFETY: null output selects the default output sink.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe { super::mi_process_info_print_out(None, ptr::null_mut()) }
}

#[repr(C)]
struct mi_json_buf_s {
    buf: *mut c_char,
    size: usize,
    used: usize,
    can_realloc: bool,
}

type mi_json_buf_t = mi_json_buf_s;

impl Default for mi_json_buf_s {
    fn default() -> Self {
        Self {
            buf: ptr::null_mut(),
            size: 0,
            used: 0,
            can_realloc: true,
        }
    }
}

fn mi_json_buf_expand(hbuf: &mut mi_json_buf_t) -> bool {
    if !hbuf.buf.is_null() && hbuf.size > 0 {
        // SAFETY: `buf` is valid for `size` bytes when non-null.
        // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
        unsafe { *hbuf.buf.add(hbuf.size - 1) = 0 };
    }
    if hbuf.size > usize::MAX / 2 || !hbuf.can_realloc {
        return false;
    }
    let newsize = if hbuf.size == 0 {
        mi_good_size_typed(RequestedSize::new(12 * 1024)).get()
    } else {
        2 * hbuf.size
    };
    // SAFETY: `mi_rezalloc` accepts null for fresh allocation and preserves the old contents.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    let newbuf = unsafe { mi_rezalloc(hbuf.buf.cast(), newsize).cast::<c_char>() };
    if newbuf.is_null() {
        return false;
    }
    hbuf.buf = newbuf;
    hbuf.size = newsize;
    true
}

fn mi_json_buf_print(hbuf: &mut mi_json_buf_t, msg: &str) {
    if hbuf.used + 1 >= hbuf.size && !hbuf.can_realloc {
        return;
    }
    for byte in msg.bytes() {
        if hbuf.used + 1 >= hbuf.size && !mi_json_buf_expand(hbuf) {
            return;
        }
        crate::internal::mi_debug_assert!(hbuf.used < hbuf.size);
        // SAFETY: capacity is ensured just above and `used < size`.
        // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
        unsafe { *hbuf.buf.add(hbuf.used) = byte as c_char };
        hbuf.used += 1;
    }
    crate::internal::mi_debug_assert!(hbuf.used < hbuf.size);
    // SAFETY: capacity is ensured above and the trailing NUL fits.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe { *hbuf.buf.add(hbuf.used) = 0 };
}

fn mi_json_buf_print_count_bin(
    hbuf: &mut mi_json_buf_t,
    prefix: &str,
    stat: &mi_stat_count_t,
    bin: StatsBin,
    add_comma: bool,
) {
    let stat = StatCountSnapshot::from_stat(stat);
    let binsize = mi_stats_get_bin_size_typed(bin);
    let pagesize = if binsize.get() <= MI_SMALL_MAX_OBJ_SIZE {
        MI_SMALL_PAGE_SIZE
    } else if binsize.get() <= MI_MEDIUM_MAX_OBJ_SIZE {
        MI_MEDIUM_PAGE_SIZE
    } else if binsize.get() <= MI_LARGE_MAX_OBJ_SIZE {
        MI_LARGE_PAGE_SIZE
    } else {
        0
    };
    let mut buf = [0 as c_char; 128];
    libc::mi_vsnprintf_args(
        &mut buf,
        format_args!(
            "{prefix}{{ \"total\": {}, \"peak\": {}, \"current\": {}, \"block_size\": {}, \"page_size\": {pagesize} }}{}\n",
            stat.total.get(),
            stat.peak.get(),
            stat.current.get(),
            binsize.get(),
            if add_comma { "," } else { "" }
        ),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_count_cbin(
    hbuf: &mut mi_json_buf_t,
    prefix: &str,
    stat: &mi_stat_count_t,
    bin: mi_chunkbin_t,
    add_comma: bool,
) {
    let stat = StatCountSnapshot::from_stat(stat);
    let cbin = match bin {
        mi_chunkbin_e::MI_CBIN_SMALL => "S",
        mi_chunkbin_e::MI_CBIN_MEDIUM => "M",
        mi_chunkbin_e::MI_CBIN_LARGE => "L",
        mi_chunkbin_e::MI_CBIN_HUGE => "H",
        mi_chunkbin_e::MI_CBIN_OTHER => "X",
        _ => " ",
    };
    let mut buf = [0 as c_char; 128];
    libc::mi_vsnprintf_args(
        &mut buf,
        format_args!(
            "{prefix}{{ \"total\": {}, \"peak\": {}, \"current\": {}, \"bin\": \"{cbin}\" }}{}\n",
            stat.total.get(),
            stat.peak.get(),
            stat.current.get(),
            if add_comma { "," } else { "" }
        ),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_count(
    hbuf: &mut mi_json_buf_t,
    prefix: &str,
    stat: &mi_stat_count_t,
    add_comma: bool,
) {
    let stat = StatCountSnapshot::from_stat(stat);
    let mut buf = [0 as c_char; 128];
    libc::mi_vsnprintf_args(
        &mut buf,
        format_args!(
            "{prefix}{{ \"total\": {}, \"peak\": {}, \"current\": {} }}{}\n",
            stat.total.get(),
            stat.peak.get(),
            stat.current.get(),
            if add_comma { "," } else { "" }
        ),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_count_value(hbuf: &mut mi_json_buf_t, name: &str, stat: &mi_stat_count_t) {
    let mut buf = [0 as c_char; 128];
    libc::mi_vsnprintf_args(&mut buf, format_args!("  \"{name}\": "));
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
    mi_json_buf_print_count(hbuf, "", stat, true);
}

#[inline(never)]
fn mi_json_buf_print_value(hbuf: &mut mi_json_buf_t, name: &str, val: i64) {
    let mut buf = [0 as c_char; 128];
    libc::mi_vsnprintf_args(&mut buf, format_args!("  \"{name}\": {val},\n"));
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_size(hbuf: &mut mi_json_buf_t, name: &str, val: usize, add_comma: bool) {
    let mut buf = [0 as c_char; 128];
    libc::mi_vsnprintf_args(
        &mut buf,
        format_args!(
            "    \"{name}\": {val}{}\n",
            if add_comma { "," } else { "" }
        ),
    );
    mi_json_buf_print(hbuf, c_char_buffer_str(&buf));
}

fn mi_json_buf_print_counter_value(hbuf: &mut mi_json_buf_t, name: &str, stat: &mi_stat_counter_t) {
    mi_json_buf_print_value(hbuf, name, StatCounterSnapshot::from_stat(stat).total.get());
}

fn append_stats_json(hbuf: &mut mi_json_buf_t, stats: &mi_stats_t) {
    macro_rules! MI_STAT_COUNT {
        ($field:ident) => {
            mi_json_buf_print_count_value(hbuf, stringify!($field), &stats.$field)
        };
    }

    MI_STAT_COUNT!(pages);
    MI_STAT_COUNT!(reserved);
    MI_STAT_COUNT!(committed);
    mi_json_buf_print_counter_value(hbuf, "reset", &stats.reset);
    mi_json_buf_print_counter_value(hbuf, "purged", &stats.purged);
    MI_STAT_COUNT!(page_committed);
    MI_STAT_COUNT!(pages_abandoned);
    MI_STAT_COUNT!(threads);
    MI_STAT_COUNT!(malloc_normal);
    MI_STAT_COUNT!(malloc_huge);
    MI_STAT_COUNT!(malloc_requested);
    mi_json_buf_print_counter_value(hbuf, "mmap_calls", &stats.mmap_calls);
    mi_json_buf_print_counter_value(hbuf, "commit_calls", &stats.commit_calls);
    mi_json_buf_print_counter_value(hbuf, "reset_calls", &stats.reset_calls);
    mi_json_buf_print_counter_value(hbuf, "purge_calls", &stats.purge_calls);
    mi_json_buf_print_counter_value(hbuf, "arena_count", &stats.arena_count);
    mi_json_buf_print_counter_value(hbuf, "malloc_normal_count", &stats.malloc_normal_count);
    mi_json_buf_print_counter_value(hbuf, "malloc_huge_count", &stats.malloc_huge_count);
    mi_json_buf_print_counter_value(hbuf, "malloc_guarded_count", &stats.malloc_guarded_count);
    mi_json_buf_print_counter_value(hbuf, "arena_rollback_count", &stats.arena_rollback_count);
    mi_json_buf_print_counter_value(hbuf, "arena_purges", &stats.arena_purges);
    mi_json_buf_print_counter_value(hbuf, "pages_extended", &stats.pages_extended);
    mi_json_buf_print_counter_value(hbuf, "pages_retire", &stats.pages_retire);
    mi_json_buf_print_counter_value(hbuf, "page_searches", &stats.page_searches);
    mi_json_buf_print_counter_value(hbuf, "page_searches_count", &stats.page_searches_count);
    MI_STAT_COUNT!(segments);
    MI_STAT_COUNT!(segments_abandoned);
    MI_STAT_COUNT!(segments_cache);
    MI_STAT_COUNT!(_segments_reserved);
    MI_STAT_COUNT!(heaps);
    MI_STAT_COUNT!(theaps);
    mi_json_buf_print_counter_value(
        hbuf,
        "pages_reclaim_on_alloc",
        &stats.pages_reclaim_on_alloc,
    );
    mi_json_buf_print_counter_value(hbuf, "pages_reclaim_on_free", &stats.pages_reclaim_on_free);
    mi_json_buf_print_counter_value(hbuf, "pages_reabandon_full", &stats.pages_reabandon_full);
    mi_json_buf_print_counter_value(
        hbuf,
        "pages_unabandon_busy_wait",
        &stats.pages_unabandon_busy_wait,
    );
    mi_json_buf_print_counter_value(hbuf, "heaps_delete_wait", &stats.heaps_delete_wait);
}

/// # Safety
/// `stats` must be null or point to live, aligned initialized stats that remain readable and stable
/// against mutation for the initial value copy. If `output_buf` is non-null, it must be uniquely
/// writable for `output_size` bytes for the duration of this call.
pub(super) unsafe fn mi_stats_get_json_from_impl(
    stats: *const mi_stats_t,
    output_size: usize,
    output_buf: *mut c_char,
) -> *mut c_char {
    if stats.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the caller keeps the source live, aligned, readable, and stable during these header
    // reads and the subsequent plain value copy.
    let (stats_size, stats_version) = unsafe { ((*stats).size, (*stats).version) };
    if stats_size != size_of::<mi_stats_t>() || stats_version != MI_STAT_VERSION {
        return ptr::null_mut();
    }
    // SAFETY: the caller excludes mutation while the initialized source value is copied. A plain
    // read supports user-owned snapshots whose raw pointer may carry shared-reference provenance.
    let snapshot = unsafe { ptr::read(stats) };
    let stats = &snapshot;

    let mut hbuf = mi_json_buf_t::default();
    if output_size > 0 && !output_buf.is_null() {
        // SAFETY: caller provides `output_buf` writable for `output_size` bytes.
        // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
        unsafe { ptr::write_bytes(output_buf, 0, output_size) };
        hbuf.buf = output_buf;
        hbuf.size = output_size;
        hbuf.can_realloc = false;
    } else if !mi_json_buf_expand(&mut hbuf) {
        return ptr::null_mut();
    }
    mi_json_buf_print(&mut hbuf, "{\n");
    mi_json_buf_print_value(&mut hbuf, "stat_version", MI_STAT_VERSION as i64);
    mi_json_buf_print_value(&mut hbuf, "mimalloc_version", MI_MALLOC_VERSION as i64);

    mi_json_buf_print(&mut hbuf, "  \"process\": {\n");
    let mut elapsed = 0usize;
    let mut user_time = 0usize;
    let mut sys_time = 0usize;
    let mut current_rss = 0usize;
    let mut peak_rss = 0usize;
    let mut current_commit = 0usize;
    let mut peak_commit = 0usize;
    let mut page_faults = 0usize;
    // SAFETY: local pointers below are valid writable outputs.
    // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
    unsafe {
        mi_process_info_impl(ProcessInfoOutput {
            elapsed_msecs: &mut elapsed,
            user_msecs: &mut user_time,
            system_msecs: &mut sys_time,
            current_rss: &mut current_rss,
            peak_rss: &mut peak_rss,
            current_commit: &mut current_commit,
            peak_commit: &mut peak_commit,
            page_faults: &mut page_faults,
        });
    }
    mi_json_buf_print_size(&mut hbuf, "elapsed_msecs", elapsed, true);
    mi_json_buf_print_size(&mut hbuf, "user_msecs", user_time, true);
    mi_json_buf_print_size(&mut hbuf, "system_msecs", sys_time, true);
    mi_json_buf_print_size(&mut hbuf, "page_faults", page_faults, true);
    mi_json_buf_print_size(&mut hbuf, "rss_current", current_rss, true);
    mi_json_buf_print_size(&mut hbuf, "rss_peak", peak_rss, true);
    mi_json_buf_print_size(&mut hbuf, "commit_current", current_commit, true);
    mi_json_buf_print_size(&mut hbuf, "commit_peak", peak_commit, false);
    mi_json_buf_print(&mut hbuf, "  },\n");

    append_stats_json(&mut hbuf, stats);

    mi_json_buf_print(&mut hbuf, "  \"malloc_bins\": [\n");
    for bin in stats_bin_iter() {
        mi_json_buf_print_count_bin(
            &mut hbuf,
            "    ",
            &stats.malloc_bins[bin.as_usize()],
            bin,
            bin.as_usize() != MI_BIN_HUGE,
        );
    }
    mi_json_buf_print(&mut hbuf, "  ],\n");
    mi_json_buf_print(&mut hbuf, "  \"page_bins\": [\n");
    for bin in stats_bin_iter() {
        mi_json_buf_print_count_bin(
            &mut hbuf,
            "    ",
            &stats.page_bins[bin.as_usize()],
            bin,
            bin.as_usize() != MI_BIN_HUGE,
        );
    }
    mi_json_buf_print(&mut hbuf, "  ],\n");
    mi_json_buf_print(&mut hbuf, "  \"chunk_bins\": [\n");
    for (i, bin) in MI_CHUNKBINS.iter().copied().enumerate() {
        mi_json_buf_print_count_cbin(
            &mut hbuf,
            "    ",
            &stats.chunk_bins[i],
            bin,
            i + 1 != MI_CHUNKBIN_COUNT,
        );
    }
    mi_json_buf_print(&mut hbuf, "  ]\n");
    mi_json_buf_print(&mut hbuf, "}\n");

    if hbuf.used >= hbuf.size {
        if hbuf.can_realloc {
            // SAFETY: the buffer came from mimalloc when `can_realloc` is true.
            // SAFETY: stats operations use validated counter, heap, subproc, buffer, callback, or C ABI witnesses.
            unsafe { mi_free(hbuf.buf.cast()) };
        }
        ptr::null_mut()
    } else {
        hbuf.buf
    }
}
