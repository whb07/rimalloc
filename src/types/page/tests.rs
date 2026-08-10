use super::*;
use core::mem::MaybeUninit;

#[test]
fn page_thread_id_splits_owner_and_flags() {
    let owner = ThreadId::new(0x1000).expect("valid thread id");
    let tagged = PageThreadId::from_parts(PageOwnerId::Thread(owner), PageFlags::IN_FULL_QUEUE);

    assert!(tagged.flags().contains(PageFlags::IN_FULL_QUEUE));
    assert!(!tagged.is_abandoned());
    assert!(!tagged.is_abandoned_mapped());
}

#[test]
fn page_thread_id_from_parts_round_trips_owner_and_flags() {
    let owner = ThreadId::new(0x3000).expect("valid thread id");
    let flags = PageFlags::new(MI_PAGE_FLAG_MASK).expect("valid flags");
    let tagged = PageThreadId::from_parts(PageOwnerId::Thread(owner), flags);

    assert_eq!(tagged.raw(), owner.get() | MI_PAGE_FLAG_MASK);
    assert_eq!(tagged.flags().bits(), flags.bits());

    let abandoned = PageThreadId::from_parts(PageOwnerId::Abandoned, flags);
    assert!(abandoned.is_abandoned());
    assert_eq!(abandoned.flags().bits(), flags.bits());

    let mapped = PageThreadId::from_parts(PageOwnerId::AbandonedMapped, flags);
    assert!(mapped.is_abandoned_mapped());
    assert_eq!(mapped.flags().bits(), flags.bits());
}

#[test]
fn page_thread_id_preserves_flags_when_owner_changes() {
    let tagged = PageThreadId::new(PageFlags::HAS_INTERIOR_POINTERS.bits())
        .with_owner(PageOwnerId::AbandonedMapped);

    assert!(tagged.is_abandoned_mapped());
    assert!(tagged.flags().contains(PageFlags::HAS_INTERIOR_POINTERS));

    let abandoned = tagged.with_owner(PageOwnerId::Abandoned);
    assert!(abandoned.is_abandoned());
    assert!(abandoned.flags().contains(PageFlags::HAS_INTERIOR_POINTERS));
}

#[test]
fn xthread_delta_classifies_fast_and_slow_paths() {
    let tid = ThreadId::new(0x2000).expect("valid thread id");
    let local = PageThreadId::new(tid.get());
    assert!(local.xor_current(tid).is_local_fast());
    assert_eq!(local.classify_free_path(tid), FreePathKind::LocalFast);

    let flagged = PageThreadId::new(tid.get() | PageFlags::IN_FULL_QUEUE.bits());
    assert!(flagged.xor_current(tid).needs_local_slow());
    assert_eq!(
        flagged.classify_free_path(tid),
        FreePathKind::LocalSlowFlags
    );

    let remote = PageThreadId::new(0x4000);
    assert!(remote.xor_current(tid).is_remote_plain());
    assert_eq!(remote.classify_free_path(tid), FreePathKind::RemotePlain);

    let remote_flagged = PageThreadId::new(0x4000 | PageFlags::IN_FULL_QUEUE.bits());
    assert_eq!(
        remote_flagged.classify_free_path(tid),
        FreePathKind::RemoteSlowFlags
    );
}

#[test]
fn free_path_classifier_matches_raw_xor_branch_rules() {
    let current = ThreadId::new(0x2000).expect("valid thread id");
    let cases = [
        PageThreadId::new(current.get()),
        PageThreadId::new(current.get() | PageFlags::IN_FULL_QUEUE.bits()),
        PageThreadId::new(0x4000),
        PageThreadId::new(0x4000 | PageFlags::HAS_INTERIOR_POINTERS.bits()),
    ];

    for tagged in cases {
        let raw_delta = current.get() ^ tagged.raw();
        let expected = if raw_delta == 0 {
            FreePathKind::LocalFast
        } else if raw_delta <= MI_PAGE_FLAG_MASK {
            FreePathKind::LocalSlowFlags
        } else if (raw_delta & MI_PAGE_FLAG_MASK) == 0 {
            FreePathKind::RemotePlain
        } else {
            FreePathKind::RemoteSlowFlags
        };

        assert_eq!(tagged.classify_free_path(current), expected);
    }
}

#[test]
fn thread_id_new_rejects_zero_and_flag_bits() {
    assert!(ThreadId::new(0).is_none());
    assert!(ThreadId::new(MI_PAGE_FLAG_MASK).is_none());
    assert_eq!(ThreadId::new(0x1000).map(ThreadId::get), Some(0x1000));
}

#[test]
fn page_flags_new_rejects_bits_outside_flag_mask() {
    assert_eq!(PageFlags::new(0).map(PageFlags::bits), Some(0));
    assert_eq!(
        PageFlags::new(MI_PAGE_FLAG_MASK).map(PageFlags::bits),
        Some(MI_PAGE_FLAG_MASK)
    );
    assert!(PageFlags::new(MI_PAGE_FLAG_MASK + 1).is_none());
}

#[test]
fn registered_page_range_preserves_registration_domain() {
    let mut raw = MaybeUninit::<crate::types::mi_page_t>::zeroed();
    // SAFETY: This fixture uses stack-local
    // page metadata only to exercise `RegisteredPageRange` witness accessors.
    let page = unsafe { PagePtr::new_unchecked(raw.as_mut_ptr()) };
    let index = PageMapIndex::new(7).expect("valid page-map index");
    let sub_index = PageMapSubIndex::new(3).expect("valid page-map sub-index");
    let slice_count = SliceCount::from_usize(5).expect("valid slice count");

    // SAFETY: The synthetic range is never
    // published to the allocator page map; the test only verifies stored
    // witness fields round-trip unchanged.
    let range = unsafe { RegisteredPageRange::new_unchecked(page, index, sub_index, slice_count) };

    assert_eq!(range.page().as_ptr(), page.as_ptr());
    assert_eq!(range.index(), index);
    assert_eq!(range.sub_index(), sub_index);
    assert_eq!(range.slice_count(), slice_count);
}
