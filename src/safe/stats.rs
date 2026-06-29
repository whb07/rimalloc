use core::ffi::CStr;
use core::fmt;
use core::ptr;
use core::ptr::NonNull;
use core::time::Duration;

use super::HeapRef;

/// Error returned by safe statistics APIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StatsError {
    /// Process-wide statistics could not be captured.
    Unavailable,
    /// Heap statistics could not be captured.
    HeapUnavailable,
    /// The caller-provided output buffer is too small.
    BufferTooSmall,
}

impl fmt::Display for StatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("mimalloc statistics are unavailable"),
            Self::HeapUnavailable => f.write_str("mimalloc heap statistics are unavailable"),
            Self::BufferTooSmall => f.write_str("statistics output buffer is too small"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StatsError {}

/// Typed statistics count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[must_use]
pub struct StatCount(i64);

impl StatCount {
    /// Creates a signed statistic count or delta.
    #[inline]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// Returns the signed statistic value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Typed statistics byte total.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[must_use]
pub struct StatTotal(usize);

impl StatTotal {
    /// Creates a byte total.
    #[inline]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the byte total.
    #[inline]
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }

    #[inline]
    const fn from_i64(value: i64) -> Self {
        if value <= 0 {
            Self(0)
        } else {
            Self(value as usize)
        }
    }
}

/// Non-printing snapshot of mimalloc statistics.
#[derive(Clone, Debug)]
#[must_use]
pub struct StatsSnapshot {
    raw: crate::stats::mi_stats_t,
}

impl StatsSnapshot {
    /// Captures a process-wide non-printing statistics snapshot.
    #[inline]
    pub fn capture() -> Result<Self, StatsError> {
        let mut raw = crate::stats::mi_stats_t::default();
        // SAFETY[AUDIT:safe-stats]: `raw` is a fresh writable stats value.
        unsafe { crate::stats::mi_stats_init(ptr::addr_of_mut!(raw)) };
        // SAFETY[AUDIT:safe-stats]: initializes allocator state for the
        // current thread before collecting process stats.
        unsafe { crate::init::mi_thread_init() };
        // SAFETY[AUDIT:safe-stats]: `raw` is valid writable stats storage.
        if unsafe { crate::stats::mi_stats_get(ptr::addr_of_mut!(raw)) } {
            Ok(Self { raw })
        } else {
            Err(StatsError::Unavailable)
        }
    }

    /// Captures a non-printing statistics snapshot for `heap`.
    #[inline]
    pub fn for_heap(heap: HeapRef<'_>) -> Result<Self, StatsError> {
        let mut raw = crate::stats::mi_stats_t::default();
        // SAFETY[AUDIT:safe-stats]: `raw` is a fresh writable stats value.
        unsafe { crate::stats::mi_stats_init(ptr::addr_of_mut!(raw)) };
        // SAFETY[AUDIT:safe-stats]: initializes allocator state for the
        // current thread before collecting heap stats.
        unsafe { crate::init::mi_thread_init() };
        // SAFETY[AUDIT:safe-stats]: `raw` is valid writable stats storage and
        // `heap` is a live borrowed heap handle.
        if unsafe { crate::stats::mi_heap_stats_get(heap.ptr.as_ptr(), ptr::addr_of_mut!(raw)) } {
            Ok(Self { raw })
        } else {
            Err(StatsError::HeapUnavailable)
        }
    }

    /// Returns total requested malloc bytes from the snapshot.
    #[inline]
    pub fn malloc_requested_total(&self) -> StatTotal {
        StatTotal::from_i64(self.raw.malloc_requested.total)
    }

    /// Returns current reserved bytes from the snapshot.
    #[inline]
    pub fn reserved_current(&self) -> StatTotal {
        StatTotal::from_i64(self.raw.reserved.current)
    }

    /// Returns the current page count from the snapshot.
    #[inline]
    pub fn pages_current(&self) -> StatCount {
        StatCount::new(self.raw.pages.current)
    }

    /// Writes this snapshot as JSON into `output`.
    #[inline]
    pub fn write_json(&self, output: &mut [u8]) -> Result<usize, StatsError> {
        if output.is_empty() {
            return Err(StatsError::BufferTooSmall);
        }
        // SAFETY[AUDIT:safe-stats]: `output` is writable for its length and
        // `self.raw` is an initialized stats snapshot.
        let ptr = unsafe {
            crate::stats::mi_stats_get_json_from(
                ptr::addr_of!(self.raw),
                output.len(),
                output.as_mut_ptr().cast(),
            )
        };
        let Some(ptr) = NonNull::new(ptr) else {
            return Err(StatsError::BufferTooSmall);
        };
        // SAFETY[AUDIT:safe-stats]: successful JSON formatting writes a NUL
        // terminated string into `output`.
        let bytes = unsafe { CStr::from_ptr(ptr.as_ptr()) }.to_bytes();
        if !bytes.starts_with(b"{") || !bytes.ends_with(b"}\n") {
            return Err(StatsError::BufferTooSmall);
        }
        Ok(bytes.len())
    }
}

/// Safe process allocator information.
///
/// Mimalloc's process-info C API reports unavailable numeric fields as zero on
/// some platforms. Size totals use `None` when the raw value is zero, so a
/// reported zero total is indistinguishable from an unavailable total through
/// this safe facade. Page faults are always returned because zero is a valid
/// observed count for that field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct ProcessInfo {
    elapsed_msecs: usize,
    user_msecs: usize,
    system_msecs: usize,
    current_rss: Option<StatTotal>,
    peak_rss: Option<StatTotal>,
    current_commit: Option<StatTotal>,
    peak_commit: Option<StatTotal>,
    page_faults: Option<StatCount>,
    page_size: Option<usize>,
}

impl ProcessInfo {
    /// Captures current process allocator information.
    #[inline]
    pub fn current() -> Self {
        let mut elapsed_msecs = 0usize;
        let mut user_msecs = 0usize;
        let mut system_msecs = 0usize;
        let mut current_rss = 0usize;
        let mut peak_rss = 0usize;
        let mut current_commit = 0usize;
        let mut peak_commit = 0usize;
        let mut page_faults = 0usize;
        // SAFETY[AUDIT:safe-process-info]: all out-pointers are valid local
        // storage for one `usize`.
        unsafe {
            crate::stats::mi_process_info(
                &mut elapsed_msecs,
                &mut user_msecs,
                &mut system_msecs,
                &mut current_rss,
                &mut peak_rss,
                &mut current_commit,
                &mut peak_commit,
                &mut page_faults,
            );
        }
        let page_size = unsafe { crate::os::_mi_os_page_size() };
        Self {
            elapsed_msecs,
            user_msecs,
            system_msecs,
            current_rss: nonzero_total(current_rss),
            peak_rss: nonzero_total(peak_rss),
            current_commit: nonzero_total(current_commit),
            peak_commit: nonzero_total(peak_commit),
            page_faults: Some(StatCount::new(page_faults.min(i64::MAX as usize) as i64)),
            page_size: (page_size != 0).then_some(page_size),
        }
    }

    /// Returns elapsed process time in raw milliseconds.
    #[inline]
    #[must_use]
    pub const fn elapsed_msecs(self) -> usize {
        self.elapsed_msecs
    }

    /// Returns elapsed process time as a [`Duration`].
    #[inline]
    #[must_use]
    pub const fn elapsed(self) -> Duration {
        Duration::from_millis(self.elapsed_msecs as u64)
    }

    /// Returns user CPU time in raw milliseconds.
    #[inline]
    #[must_use]
    pub const fn user_msecs(self) -> usize {
        self.user_msecs
    }

    /// Returns user CPU time as a [`Duration`].
    #[inline]
    #[must_use]
    pub const fn user_time(self) -> Duration {
        Duration::from_millis(self.user_msecs as u64)
    }

    /// Returns system CPU time in raw milliseconds.
    #[inline]
    #[must_use]
    pub const fn system_msecs(self) -> usize {
        self.system_msecs
    }

    /// Returns system CPU time as a [`Duration`].
    #[inline]
    #[must_use]
    pub const fn system_time(self) -> Duration {
        Duration::from_millis(self.system_msecs as u64)
    }

    /// Returns current resident set size when available.
    #[inline]
    #[must_use]
    pub const fn current_rss(self) -> Option<StatTotal> {
        self.current_rss
    }

    /// Returns peak resident set size when available.
    #[inline]
    #[must_use]
    pub const fn peak_rss(self) -> Option<StatTotal> {
        self.peak_rss
    }

    /// Returns current committed memory when available.
    #[inline]
    #[must_use]
    pub const fn current_commit(self) -> Option<StatTotal> {
        self.current_commit
    }

    /// Returns peak committed memory when available.
    #[inline]
    #[must_use]
    pub const fn peak_commit(self) -> Option<StatTotal> {
        self.peak_commit
    }

    /// Returns process page-fault count when available.
    #[inline]
    #[must_use]
    pub const fn page_faults(self) -> Option<StatCount> {
        self.page_faults
    }

    /// Returns OS page size when available.
    #[inline]
    #[must_use]
    pub const fn page_size(self) -> Option<usize> {
        self.page_size
    }
}

#[inline]
const fn nonzero_total(value: usize) -> Option<StatTotal> {
    if value == 0 {
        None
    } else {
        Some(StatTotal::new(value))
    }
}
