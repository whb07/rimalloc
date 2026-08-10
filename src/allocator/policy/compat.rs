use core::ffi::c_long;
use core::num::NonZeroU16;

use crate::allocator::options::{_mi_option_get_fast, mi_option_get, mi_option_t};
use crate::allocator::page::MI_SMALL_MAX_OBJ_SIZE;
use crate::core_types::{
    BlockSize, PageCapacity, PageReclaimLimit, PageReclaimOnFree, ReservedBlockCount, RetireExpire,
};
use crate::types::MI_SMALL_SIZE_MAX;

use super::{
    GenericMaintenanceInterval, PageExtendCount, PageFullRetain, ReclaimContext, RemoteCollectMode,
    ThreadHeapPolicy,
};

/// The exact tuning decisions used by the vendored mimalloc-compatible behavior.
pub(crate) struct CompatPolicy;

pub(super) const MAX_EXTEND_SIZE: usize = 4 * 1024;
pub(super) const MIN_EXTEND: usize = if crate::internal::MI_SECURE != 0 {
    8 * crate::internal::MI_SECURE
} else {
    1
};

const fn clamp_long(value: c_long, min: c_long, max: c_long) -> c_long {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

impl CompatPolicy {
    #[inline(always)]
    pub(crate) const fn generic_maintenance_interval() -> GenericMaintenanceInterval {
        GenericMaintenanceInterval::new(1000)
    }

    #[inline(always)]
    pub(crate) fn generic_collect_limit() -> GenericMaintenanceInterval {
        Self::generic_collect_limit_from_raw(mi_option_get(mi_option_t::mi_option_generic_collect))
    }

    pub(super) const fn generic_collect_limit_from_raw(raw: c_long) -> GenericMaintenanceInterval {
        GenericMaintenanceInterval::new(clamp_long(raw, 1, 1_000_000))
    }

    #[inline]
    pub(crate) fn thread_heap_policy() -> ThreadHeapPolicy {
        let reclaim_on_free = mi_option_get(mi_option_t::mi_option_page_reclaim_on_free);
        let allow_abandon = mi_option_get(mi_option_t::mi_option_page_full_retain);
        let full_retain = mi_option_get(mi_option_t::mi_option_page_full_retain);
        Self::thread_heap_policy_from_raw(reclaim_on_free, allow_abandon, full_retain)
    }

    pub(super) const fn thread_heap_policy_from_raw(
        reclaim_on_free: c_long,
        allow_abandon: c_long,
        full_retain: c_long,
    ) -> ThreadHeapPolicy {
        ThreadHeapPolicy::new(
            reclaim_on_free >= 0,
            allow_abandon >= 0,
            PageFullRetain::new(clamp_long(full_retain, -1, 32)),
        )
    }

    #[inline(always)]
    pub(crate) const fn adjust_for_thread_pool(policy: ThreadHeapPolicy) -> ThreadHeapPolicy {
        let retain = policy.page_full_retain().get();
        ThreadHeapPolicy::new(
            policy.allow_page_reclaim(),
            policy.allow_page_abandon(),
            PageFullRetain::new(if retain > 0 { retain / 4 } else { retain }),
        )
    }

    #[inline(always)]
    pub(crate) const fn page_extend_count(
        block_size: BlockSize,
        capacity: PageCapacity,
        reserved: ReservedBlockCount,
    ) -> Option<PageExtendCount> {
        let remaining = reserved.get().saturating_sub(capacity.get());
        let mut max_extend = if block_size.get() >= MAX_EXTEND_SIZE {
            MIN_EXTEND
        } else {
            MAX_EXTEND_SIZE / block_size.get()
        };
        if max_extend < MIN_EXTEND {
            max_extend = MIN_EXTEND;
        }
        let extend = if (remaining as usize) < max_extend {
            remaining
        } else {
            max_extend as u16
        };
        match NonZeroU16::new(extend) {
            Some(extend) => Some(PageExtendCount::new(extend)),
            None => None,
        }
    }

    #[inline(always)]
    pub(crate) const fn initial_full_retain(
        block_size: BlockSize,
        configured: PageFullRetain,
    ) -> PageReclaimLimit {
        if block_size.get() > MI_SMALL_MAX_OBJ_SIZE {
            PageReclaimLimit::new(0)
        } else {
            PageReclaimLimit::new(configured.get())
        }
    }

    #[inline(always)]
    pub(crate) fn page_candidate_limit() -> PageReclaimLimit {
        Self::page_candidate_limit_from_raw(_mi_option_get_fast(
            mi_option_t::mi_option_page_max_candidates,
        ))
    }

    pub(super) const fn page_candidate_limit_from_raw(raw: c_long) -> PageReclaimLimit {
        PageReclaimLimit::new(raw)
    }

    #[inline(always)]
    pub(crate) const fn retire_expire(block_size: BlockSize) -> RetireExpire {
        RetireExpire::new(if block_size.get() <= MI_SMALL_MAX_OBJ_SIZE {
            16
        } else {
            4
        })
    }

    #[inline(always)]
    pub(crate) fn reclaim_on_free_mode() -> PageReclaimOnFree {
        Self::reclaim_on_free_mode_from_raw(_mi_option_get_fast(
            mi_option_t::mi_option_page_reclaim_on_free,
        ))
    }

    pub(super) const fn reclaim_on_free_mode_from_raw(raw: c_long) -> PageReclaimOnFree {
        PageReclaimOnFree::new(raw)
    }

    #[inline(always)]
    pub(crate) fn reclaim_limit(context: ReclaimContext) -> PageReclaimLimit {
        let option = match context {
            ReclaimContext::SameThreadHeap => mi_option_t::mi_option_page_max_reclaim,
            ReclaimContext::CrossThreadHeap | ReclaimContext::ThreadPool => {
                mi_option_t::mi_option_page_cross_thread_max_reclaim
            }
        };
        Self::reclaim_limit_from_raw(_mi_option_get_fast(option))
    }

    pub(super) const fn reclaim_limit_from_raw(raw: c_long) -> PageReclaimLimit {
        PageReclaimLimit::new(raw)
    }

    #[inline(always)]
    pub(crate) const fn remote_collect_mode(block_size: BlockSize) -> RemoteCollectMode {
        if block_size.get() <= MI_SMALL_SIZE_MAX {
            RemoteCollectMode::PartialKeepPublishedHead
        } else {
            RemoteCollectMode::FullDetach
        }
    }
}
