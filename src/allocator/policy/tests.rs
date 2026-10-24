use core::ffi::c_long;

use crate::allocator::page::{MI_MEDIUM_MAX_OBJ_SIZE, MI_SMALL_MAX_OBJ_SIZE};
use crate::core_types::{
    BlockSize, PageCapacity, PageReclaimLimit, ReservedBlockCount, UsedBlockCount,
};

use super::compat::MIN_EXTEND;
use super::{CompatPolicy, reference};

const OPTION_VALUES: [c_long; 13] = [
    c_long::MIN,
    -2,
    -1,
    0,
    1,
    2,
    4,
    32,
    33,
    1000,
    10_000,
    1_000_000,
    c_long::MAX,
];

fn block_size(value: usize) -> BlockSize {
    unsafe { BlockSize::new_unchecked(value) }
}

#[test]
fn page_extension_matches_pre_extraction_formula() {
    let block_sizes = [
        1,
        MIN_EXTEND,
        8,
        16,
        64,
        1024,
        4095,
        4096,
        4097,
        MI_SMALL_MAX_OBJ_SIZE,
        MI_SMALL_MAX_OBJ_SIZE + 1,
        MI_MEDIUM_MAX_OBJ_SIZE,
    ];
    // The formula can observe capacity/reserved only through their difference.
    // Exhaust every possible difference and translate it to the low, midpoint,
    // and high valid capacity so subtraction boundaries are covered too.
    #[cfg(not(miri))]
    let remaining_values = 0..=u16::MAX;
    // Native tests exhaust every count; the interpreter covers arithmetic
    // boundaries while also varying addresses and schedules across Miri seeds.
    #[cfg(miri)]
    let remaining_values = [
        0,
        1,
        2,
        7,
        8,
        15,
        16,
        31,
        32,
        63,
        64,
        127,
        128,
        255,
        256,
        4095,
        4096,
        32767,
        u16::MAX - 1,
        u16::MAX,
    ];
    for remaining in remaining_values {
        let max_capacity = u16::MAX - remaining;
        for capacity in [0, max_capacity / 2, max_capacity] {
            let reserved = capacity + remaining;
            let capacity = PageCapacity::new(
                UsedBlockCount::new(0),
                capacity,
                ReservedBlockCount::new(reserved),
            )
            .expect("ordered page counts");
            for raw_block_size in block_sizes {
                let actual = CompatPolicy::page_extend_count(
                    block_size(raw_block_size),
                    capacity,
                    ReservedBlockCount::new(reserved),
                )
                .map_or(0, super::PageExtendCount::get);
                assert_eq!(
                    actual,
                    reference::extend_count(raw_block_size, capacity.get(), reserved, MIN_EXTEND,)
                );
            }
        }
    }
}

#[test]
fn option_interpretation_preserves_clamps_and_signed_sentinels() {
    for raw in OPTION_VALUES {
        assert_eq!(
            CompatPolicy::generic_collect_limit_from_raw(raw).get(),
            raw.clamp(1, 1_000_000)
        );
        assert_eq!(
            CompatPolicy::page_candidate_limit_from_raw(raw),
            PageReclaimLimit::new(raw)
        );
        assert_eq!(
            CompatPolicy::reclaim_limit_from_raw(raw),
            PageReclaimLimit::new(raw)
        );
        assert_eq!(
            CompatPolicy::reclaim_on_free_mode_from_raw(raw).is_enabled(),
            raw >= 0
        );
        let actual = CompatPolicy::thread_heap_policy_from_raw(raw, raw, raw);
        let expected = reference::thread_heap_options(raw, raw, false);
        assert_eq!(
            (
                actual.allow_page_reclaim(),
                actual.allow_page_abandon(),
                actual.page_full_retain().get(),
            ),
            expected
        );
    }
}

#[test]
fn thread_pool_retain_adjustment_matches_integer_division() {
    for raw in [-1, 0, 1, 2, 3, 4, 32] {
        let policy = CompatPolicy::thread_heap_policy_from_raw(0, raw, raw);
        let adjusted = CompatPolicy::adjust_for_thread_pool(policy);
        assert_eq!(
            adjusted.page_full_retain().get(),
            reference::thread_heap_options(0, raw, true).2
        );
    }
}

#[test]
fn thread_heap_policy_keeps_independent_option_reads_distinct() {
    let policy = CompatPolicy::thread_heap_policy_from_raw(-1, 0, 33);
    assert!(!policy.allow_page_reclaim());
    assert!(policy.allow_page_abandon());
    assert_eq!(policy.page_full_retain().get(), 32);
}

#[test]
fn retirement_matches_pre_extraction_formula() {
    for raw in [
        1,
        MI_SMALL_MAX_OBJ_SIZE,
        MI_SMALL_MAX_OBJ_SIZE + 1,
        MI_MEDIUM_MAX_OBJ_SIZE,
    ] {
        assert_eq!(
            CompatPolicy::retire_expire(block_size(raw)).get(),
            reference::retire_expire(raw, MI_SMALL_MAX_OBJ_SIZE)
        );
    }
}

#[test]
fn remote_collection_switches_at_the_compatibility_boundary() {
    assert_eq!(
        CompatPolicy::remote_collect_mode(block_size(crate::types::MI_SMALL_SIZE_MAX)),
        super::RemoteCollectMode::PartialKeepPublishedHead
    );
    assert_eq!(
        CompatPolicy::remote_collect_mode(block_size(crate::types::MI_SMALL_SIZE_MAX + 1)),
        super::RemoteCollectMode::FullDetach
    );
}

#[cfg(feature = "bench-diagnostics")]
#[test]
fn option_dependent_policy_calls_preserve_fast_read_counts() {
    let _guard = crate::allocator::bench_diagnostics::test_support::lock();
    crate::allocator::bench_diagnostics::mi_bench_diagnostics_reset();
    let _ = CompatPolicy::page_candidate_limit();
    let _ = CompatPolicy::reclaim_on_free_mode();
    let _ = CompatPolicy::reclaim_limit(super::ReclaimContext::SameThreadHeap);
    let _ = CompatPolicy::reclaim_limit(super::ReclaimContext::CrossThreadHeap);
    assert_eq!(
        crate::allocator::bench_diagnostics::option_read_counts_for_tests(),
        (4, 1, 1, 1, 1)
    );
}
