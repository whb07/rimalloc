use super::*;

#[test]
fn requested_size_padding_checks_overflow() {
    assert!(RequestedSize::new(16).checked_padded().is_ok());
    assert_eq!(
        RequestedSize::new(usize::MAX).checked_padded(),
        Err(AllocSizeError::Overflow)
    );
}

#[test]
fn usable_size_checked_sub_reports_underflow() {
    assert_eq!(UsableSize::new(16).checked_sub(4), Ok(UsableSize::new(12)));
    assert_eq!(
        UsableSize::new(3).checked_sub(4),
        Err(AllocSizeError::Overflow)
    );
}

#[test]
fn scalar_constructors_preserve_zero_and_upper_bound_values() {
    assert_eq!(RequestedSize::new(0).get(), 0);
    assert_eq!(RequestedSize::new(usize::MAX).get(), usize::MAX);
    assert_eq!(UsableSize::new(0).get(), 0);
    assert_eq!(UsableSize::new(usize::MAX).get(), usize::MAX);
    assert_eq!(AlignmentOffset::new(usize::MAX).get(), usize::MAX);
    assert_eq!(PageQueueCount::new(usize::MAX).get(), usize::MAX);
    assert_eq!(UsedBlockCount::new(u16::MAX).get(), u16::MAX);
    assert_eq!(ReservedBlockCount::new(u16::MAX).get(), u16::MAX);
    assert_eq!(RetireExpire::new(u8::MAX).get(), u8::MAX);
    assert_eq!(NumaNode::new(i32::MAX).get(), i32::MAX);
    assert_eq!(NumaNode::new(i32::MIN).get(), i32::MIN);
    assert_eq!(ThreadSeq::new(usize::MAX).get(), usize::MAX);
    assert_eq!(HeapSeq::new(usize::MAX).get(), usize::MAX);
    assert_eq!(SubprocSeq::new(usize::MAX).get(), usize::MAX);
    assert_eq!(Heartbeat::new(u64::MAX).get(), u64::MAX);
    assert_eq!(SliceIndex::new(usize::MAX).get(), usize::MAX);
    assert_eq!(GoodSize::new(usize::MAX).get(), usize::MAX);
}

#[test]
fn slice_index_checked_add_reports_overflow() {
    assert_eq!(
        SliceIndex::new(3).checked_add(4).map(SliceIndex::get),
        Some(7)
    );
    assert_eq!(SliceIndex::new(usize::MAX).checked_add(1), None);
}

#[test]
fn scalar_constructor_boundaries_cover_overflow_and_limits() {
    let max_power_of_two = 1usize << (usize::BITS - 1);
    assert_eq!(
        Alignment::new(max_power_of_two).map(Alignment::get),
        Some(max_power_of_two)
    );
    assert_eq!(
        MaybeAlignment::new(max_power_of_two).map(MaybeAlignment::get),
        Some(max_power_of_two)
    );

    let max_capacity = PageCapacity::new(
        UsedBlockCount::new(u16::MAX),
        u16::MAX,
        ReservedBlockCount::new(u16::MAX),
    )
    .expect("u16 max is valid when used and reserved match");
    assert_eq!(max_capacity.get(), u16::MAX);
    assert!(
        max_capacity
            .add(
                UsedBlockCount::new(u16::MAX),
                1,
                ReservedBlockCount::new(u16::MAX)
            )
            .is_none()
    );

    assert_eq!(
        SliceCount::from_usize(u32::MAX as usize).map(SliceCount::get),
        Some(u32::MAX)
    );
    assert!(SliceCount::from_usize(u32::MAX as usize + 1).is_none());
}

#[test]
fn block_size_usable_subtracts_padding_domain() {
    let raw = crate::types::MI_PADDING_SIZE + 64;
    let block = unsafe { BlockSize::new_unchecked(raw) };
    assert_eq!(block.usable().get(), 64);
}

#[test]
fn alignment_offset_preserves_raw_offset_domain() {
    assert_eq!(AlignmentOffset::new(0).get(), 0);
    assert_eq!(AlignmentOffset::new(17).get(), 17);
}

#[test]
fn wsize_rounds_byte_size_up_to_words() {
    use crate::internal::_mi_wsize_from_size;
    let word = core::mem::size_of::<usize>();
    assert_eq!(_mi_wsize_from_size(0), 0);
    assert_eq!(_mi_wsize_from_size(1), 1);
    assert_eq!(_mi_wsize_from_size(word), 1);
    assert_eq!(_mi_wsize_from_size(word + 1), 2);
}

#[test]
fn page_queue_count_arithmetic_preserves_domain() {
    let count = PageQueueCount::new(1).increment();
    assert_eq!(count.get(), 2);
    assert_eq!(count.decrement().get(), 1);
}

#[test]
fn page_count_domains_preserve_raw_counts() {
    let used = UsedBlockCount::new(3);
    let reserved = ReservedBlockCount::new(9);

    assert_eq!(used.get(), 3);
    assert_eq!(used.as_usize(), 3);
    assert_eq!(used.increment_wrapping().get(), 4);
    assert_eq!(used.decrement().get(), 2);

    assert_eq!(reserved.get(), 9);
    assert_eq!(reserved.as_usize(), 9);
}

#[test]
fn page_capacity_checks_used_capacity_reserved_ordering() {
    let used = UsedBlockCount::new(3);
    let reserved = ReservedBlockCount::new(9);

    let capacity = PageCapacity::new(used, 6, reserved).expect("valid page capacity");
    assert_eq!(capacity.get(), 6);
    assert_eq!(capacity.as_usize(), 6);
    assert_eq!(
        capacity.add(used, 2, reserved).map(PageCapacity::get),
        Some(8)
    );

    assert!(PageCapacity::new(UsedBlockCount::new(7), 6, reserved).is_none());
    assert!(PageCapacity::new(used, 10, reserved).is_none());
    assert!(capacity.add(used, 4, reserved).is_none());
}

#[test]
fn retire_expire_models_zero_and_countdown_values() {
    assert!(RetireExpire::zero().is_zero());
    assert_eq!(RetireExpire::zero().get(), 0);

    let expire = RetireExpire::new(3);
    assert!(!expire.is_zero());
    assert_eq!(expire.decrement().get(), 2);
}

#[test]
fn numa_node_models_any_and_normalized_affinity() {
    let any = NumaNode::any();
    let node = NumaNode::new(5);

    assert_eq!(any.get(), -1);
    assert!(any.is_any());
    assert!(!node.is_any());
    assert!(NumaNode::new(2).is_suitable_for(any));
    assert!(any.is_suitable_for(node));
    assert!(NumaNode::new(5).is_suitable_for(node));
    assert!(!NumaNode::new(4).is_suitable_for(node));
    assert_eq!(node.normalize(4).get(), 1);
    assert_eq!(node.normalize(0).get(), -1);
}

#[test]
fn sequence_domains_preserve_raw_values() {
    assert_eq!(ThreadSeq::new(11).get(), 11);
    assert_eq!(HeapSeq::new(13).get(), 13);
    assert_eq!(SubprocSeq::new(17).get(), 17);
    assert_eq!(ArenaCount::new(19).get(), 19);
}

#[test]
fn heartbeat_preserves_raw_value_and_wraps_on_increment() {
    assert_eq!(Heartbeat::new(41).increment_wrapping().get(), 42);
    assert_eq!(Heartbeat::new(u64::MAX).increment_wrapping().get(), 0);
}

#[test]
fn os_page_size_preserves_nonzero_raw_value() {
    let page_size = unsafe { OsPageSize::new_unchecked(4096) };
    assert_eq!(page_size.get(), 4096);
}

#[test]
fn errno_preserves_raw_error_codes() {
    assert_eq!(Errno::new(crate::types::EINVAL).get(), crate::types::EINVAL);
    assert_eq!(Errno::EAGAIN.get(), crate::types::EAGAIN);
    assert_eq!(Errno::ENOMEM.get(), crate::types::ENOMEM);
    assert_eq!(Errno::EFAULT.get(), crate::types::EFAULT);
    assert_eq!(Errno::EINVAL.get(), crate::types::EINVAL);
    assert_eq!(Errno::EOVERFLOW.get(), crate::types::EOVERFLOW);
}

#[test]
fn bin_rejects_indexes_outside_page_queue_table() {
    assert_eq!(Bin::new(0).map(|bin| bin.0), Some(0));
    assert_eq!(
        Bin::new((crate::types::MI_BIN_COUNT - 1) as u16).map(|bin| bin.0),
        Some((crate::types::MI_BIN_COUNT - 1) as u16)
    );
    assert!(Bin::new(crate::types::MI_BIN_COUNT as u16).is_none());

    let unchecked = unsafe { Bin::new_unchecked(3) };
    assert_eq!(unchecked.0, 3);
}

#[test]
fn stats_bin_rejects_indexes_above_huge_bin() {
    assert_eq!(StatsBin::new(0).map(StatsBin::get), Some(0));
    assert_eq!(
        StatsBin::new(crate::allocator::stats::MI_BIN_HUGE as u16).map(StatsBin::get),
        Some(crate::allocator::stats::MI_BIN_HUGE as u16)
    );
    assert!(StatsBin::new((crate::allocator::stats::MI_BIN_HUGE + 1) as u16).is_none());

    let unchecked = unsafe { StatsBin::new_unchecked(3) };
    assert_eq!(unchecked.as_usize(), 3);
}

#[test]
fn arena_index_rejects_indexes_outside_arena_table() {
    assert_eq!(ArenaIndex::new(0).map(ArenaIndex::get), Some(0));
    assert_eq!(
        ArenaIndex::new((crate::types::MI_MAX_ARENAS - 1) as u16).map(ArenaIndex::get),
        Some((crate::types::MI_MAX_ARENAS - 1) as u16)
    );
    assert!(ArenaIndex::new(crate::types::MI_MAX_ARENAS as u16).is_none());
    assert_eq!(ArenaIndex::from_usize(1).map(ArenaIndex::get), Some(1));
    assert!(ArenaIndex::from_usize(crate::types::MI_MAX_ARENAS).is_none());
    assert!(ArenaIndex::from_usize(u16::MAX as usize + 1).is_none());

    let unchecked = unsafe { ArenaIndex::new_unchecked(3) };
    assert_eq!(unchecked.get(), 3);
}

#[test]
fn maybe_alignment_accepts_zero_or_power_of_two() {
    assert_eq!(MaybeAlignment::new(0).map(MaybeAlignment::get), Some(0));
    assert_eq!(MaybeAlignment::new(64).map(MaybeAlignment::get), Some(64));
    assert!(MaybeAlignment::new(24).is_none());

    let unchecked = unsafe { MaybeAlignment::new_unchecked(24) };
    assert_eq!(unchecked.get(), 24);
}

#[test]
fn page_map_indexes_enforce_domain_bounds() {
    assert_eq!(PageMapIndex::new(0).map(PageMapIndex::get), Some(0));
    assert_eq!(
        PageMapIndex::new(crate::internal::MI_PAGE_MAP_COUNT - 1).map(PageMapIndex::get),
        Some(crate::internal::MI_PAGE_MAP_COUNT - 1)
    );
    assert!(PageMapIndex::new(crate::internal::MI_PAGE_MAP_COUNT).is_none());
    assert!(PageMapIndex::new(usize::MAX).is_none());

    assert_eq!(PageMapSubIndex::new(0).map(PageMapSubIndex::get), Some(0));
    assert_eq!(
        PageMapSubIndex::new(crate::internal::MI_PAGE_MAP_SUB_COUNT - 1).map(PageMapSubIndex::get),
        Some(crate::internal::MI_PAGE_MAP_SUB_COUNT - 1)
    );
    assert!(PageMapSubIndex::new(crate::internal::MI_PAGE_MAP_SUB_COUNT).is_none());
    assert!(PageMapSubIndex::new(usize::MAX).is_none());
}

#[cfg(target_pointer_width = "32")]
#[test]
fn flat_page_map_index_covers_all_32_bit_slice_slots() {
    assert_eq!(PageMapFlatIndex::new(0).map(PageMapFlatIndex::get), Some(0));
    assert_eq!(PageMapIndex::new(16), None);
    assert_eq!(
        PageMapFlatIndex::new(16).map(PageMapFlatIndex::get),
        Some(16)
    );
    assert_eq!(
        PageMapFlatIndex::new(PageMapFlatIndex::COUNT - 1).map(PageMapFlatIndex::get),
        Some(PageMapFlatIndex::COUNT - 1)
    );
    assert!(PageMapFlatIndex::new(PageMapFlatIndex::COUNT).is_none());
    assert!(PageMapFlatIndex::new(usize::MAX).is_none());
}

#[test]
fn alignment_accepts_only_nonzero_power_of_two() {
    assert_eq!(Alignment::new(1).map(Alignment::get), Some(1));
    assert_eq!(Alignment::new(64).map(Alignment::get), Some(64));
    assert!(Alignment::new(0).is_none());
    assert!(Alignment::new(24).is_none());
}

#[test]
fn slice_count_rejects_zero_and_too_large_values() {
    assert!(SliceCount::from_usize(0).is_none());
    assert_eq!(SliceCount::from_usize(7).map(SliceCount::get), Some(7));
    assert!(SliceCount::from_usize(usize::MAX).is_none());
    assert!(SliceCount::from_usize_const(0).is_none());
    assert_eq!(
        SliceCount::from_usize_const(7).map(SliceCount::get),
        Some(7)
    );
    assert!(SliceCount::from_usize_const(u32::MAX as usize + 1).is_none());
}
