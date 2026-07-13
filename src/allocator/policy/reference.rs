use core::ffi::c_long;

pub(super) const fn extend_count(
    raw_block_size: usize,
    raw_capacity: u16,
    raw_reserved: u16,
    min_extend: usize,
) -> u16 {
    let remaining = (raw_reserved - raw_capacity) as usize;
    let mut max_extend = if raw_block_size >= 4096 {
        min_extend
    } else {
        4096 / raw_block_size
    };
    if max_extend < min_extend {
        max_extend = min_extend;
    }
    if remaining < max_extend {
        remaining as u16
    } else {
        max_extend as u16
    }
}

pub(super) const fn retire_expire(raw_block_size: usize, small_max: usize) -> u8 {
    if raw_block_size <= small_max { 16 } else { 4 }
}

pub(super) const fn thread_heap_options(
    reclaim_on_free: c_long,
    full_retain: c_long,
    in_thread_pool: bool,
) -> (bool, bool, c_long) {
    let mut retain = if full_retain < -1 {
        -1
    } else if full_retain > 32 {
        32
    } else {
        full_retain
    };
    if in_thread_pool && retain > 0 {
        retain /= 4;
    }
    (reclaim_on_free >= 0, full_retain >= 0, retain)
}
