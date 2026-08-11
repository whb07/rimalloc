use super::*;
use core::mem::{MaybeUninit, align_of, size_of};

#[test]
fn block_ptr_free_list_link_helpers_delegate_to_encoded_links() {
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY: the page pointer wraps
    // stack-local page storage used only by this free-list fixture.
    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };
    // SAFETY: key writes mutate only the
    // stack-local page fixture and avoid encoded-key randomness in this test.
    unsafe { page.set_keys(PageKeys::disabled()) };

    let mut first = MaybeUninit::<mi_block_t>::zeroed();
    let mut second = MaybeUninit::<mi_block_t>::zeroed();
    // SAFETY: both block pointers wrap
    // stack-local block fixtures that stay live through the link assertions.
    let first = unsafe { BlockPtr::new_unchecked(first.as_mut_ptr()) };
    // SAFETY: `second` wraps stack-local
    // block storage used only as an encoded next pointer target.
    let second = unsafe { BlockPtr::new_unchecked(second.as_mut_ptr()) };

    // SAFETY: link writes target stack-local
    // block metadata using the live stack-local page key fixture.
    unsafe { first.set_next(page, Some(second)) };
    // SAFETY: the encoded link was written
    // above and both block fixtures remain live.
    unsafe {
        assert_eq!(
            first.next(page).map(BlockPtr::as_ptr),
            Some(second.as_ptr())
        );
    }

    // SAFETY: link clearing mutates only the
    // stack-local first block fixture.
    unsafe { first.set_next(page, None) };
    // SAFETY: the encoded link was just
    // cleared and the first block fixture remains live.
    unsafe {
        assert!(first.next(page).is_none());
    }
}

#[test]
fn provenance_address_helpers_match_raw_pointer_math() {
    let bytes = [0u8; 16];
    let base = bytes.as_ptr();
    // SAFETY: offset 9 is within the live
    // stack-local byte buffer and is used only for address-difference checks.
    let at_9 = unsafe { base.add(9) };

    assert_eq!(ptr_exposed_addr(base), base as usize);
    assert_eq!(ptr_addr_diff(at_9, base), 9);
    assert!(ptr_is_aligned(base, 1));
    assert!(ptr_is_aligned(base, 0));
}

#[test]
fn page_area_uses_checked_half_open_address_bounds() {
    let mut bytes = [0u8; 8];
    // SAFETY: the non-null start points into live stack storage; PageArea only
    // performs exposed-address calculations and creates no references.
    let start = unsafe { PageStart::new_unchecked(bytes.as_mut_ptr()) };
    let start_address = ptr_exposed_addr(start.as_ptr());

    let zero = PageArea::from_start_and_len(start, 0).expect("zero-length range is representable");
    assert_eq!(zero.start_address(), start_address);
    assert_eq!(zero.end_address(), start_address);
    assert!(!zero.contains(start.as_ptr()));
    assert!(!zero.contains(core::ptr::null::<u8>()));

    let minimum = PageArea::from_start_and_len(start, 1).expect("one-byte range is representable");
    assert!(minimum.contains(start.as_ptr()));
    // SAFETY: one-past is valid for address comparison and is not dereferenced.
    let one_past_minimum = unsafe { start.as_ptr().add(1) };
    assert!(!minimum.contains(one_past_minimum));

    let area = PageArea::from_start_and_len(start, bytes.len()).expect("stack range is valid");
    // SAFETY: both pointers stay within or one-past the stack byte buffer and
    // are used only for address-domain checks.
    unsafe {
        assert_eq!(area.offset_of(start.as_ptr().add(1)), Some(1));
        assert!(area.contains(start.as_ptr().add(bytes.len() - 1)));
        assert!(!area.contains(start.as_ptr().add(bytes.len())));
    }

    let mut aligned_words = [0usize; 2];
    // SAFETY: adding one byte stays within the live stack buffer. PageArea is
    // geometry-only, so an intentionally misaligned start and interior address
    // remain valid inputs for half-open address checks.
    unsafe {
        let misaligned = PageStart::new_unchecked(aligned_words.as_mut_ptr().cast::<u8>().add(1));
        assert!(!ptr_exposed_addr(misaligned.as_ptr()).is_multiple_of(size_of::<usize>()));
        let misaligned_area = PageArea::from_start_and_len(misaligned, size_of::<usize>())
            .expect("misaligned exposed-address interval does not overflow");
        assert_eq!(
            misaligned_area.offset_of(misaligned.as_ptr().add(3)),
            Some(3)
        );
    }

    // PageArea is geometry-only, so this intentionally synthetic maximum
    // interval tests integer bounds without claiming backing allocation bytes.
    let max_len = usize::MAX - start_address;
    let maximum = PageArea::from_start_and_len(start, max_len)
        .expect("largest non-overflowing exposed-address range is valid");
    assert_eq!(maximum.end_address(), usize::MAX);
    assert!(PageArea::from_start_and_len(start, max_len + 1).is_none());
}

#[test]
fn page_area_from_page_checks_size_multiplication() {
    let mut bytes = [0u8; 8];
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY: the stack page fixture is initialized for exactly the metadata
    // fields read by PageArea and remains live through all range checks.
    unsafe {
        let raw = raw_page.as_mut_ptr();
        ptr::addr_of_mut!((*raw).page_start).write(bytes.as_mut_ptr());
        ptr::addr_of_mut!((*raw).block_size).write(4);
        ptr::addr_of_mut!((*raw).reserved).write(2);
        let page = PagePtr::new_unchecked(raw);
        let area = page.area().expect("4 * 2 byte page area is valid");
        assert_eq!(area.start().as_ptr(), bytes.as_mut_ptr());
        assert_eq!(area.byte_len(), bytes.len());

        ptr::addr_of_mut!((*raw).reserved).write(0);
        assert_eq!(
            page.area().expect("zero reserved area is valid").byte_len(),
            0
        );

        ptr::addr_of_mut!((*raw).block_size).write(usize::MAX);
        ptr::addr_of_mut!((*raw).reserved).write(2);
        assert!(page.area().is_none());
    }
}

#[test]
fn page_block_layout_bounds_block_indexes_and_addresses() {
    assert_eq!(size_of::<PageBlockRange>(), 3 * size_of::<usize>());
    let mut blocks = [MaybeUninit::<mi_block_t>::uninit(); 4];
    // SAFETY: the aligned stack allocation remains live while the layout only
    // derives raw pointers within its complete four-block range.
    let start = unsafe { PageStart::new_unchecked(blocks.as_mut_ptr().cast::<u8>()) };
    // SAFETY: `mi_block_t` has a non-zero size.
    let block_size = unsafe { BlockSize::new_unchecked(size_of::<mi_block_t>()) };
    let layout = PageBlockLayout::from_parts(start, block_size, ReservedBlockCount::new(4))
        .expect("aligned four-block layout is valid");
    let range = layout
        .nonempty_range(0, 4)
        .expect("complete four-block range is valid");

    assert_eq!(layout.area().byte_len(), core::mem::size_of_val(&blocks));
    assert_eq!(
        range.first().as_ptr(),
        blocks.as_mut_ptr().cast::<mi_block_t>()
    );
    assert_eq!(
        range.last().as_ptr(),
        blocks.as_mut_ptr().wrapping_add(3).cast::<mi_block_t>()
    );
    assert!(layout.nonempty_range(4, 1).is_none());
    assert!(layout.nonempty_range(usize::MAX, 1).is_none());
    assert!(layout.nonempty_range(0, 0).is_none());

    let single = layout
        .nonempty_range(0, 1)
        .expect("minimum non-empty block range is valid");
    assert_eq!(single.first(), single.last());

    let empty = PageBlockLayout::from_parts(start, block_size, ReservedBlockCount::new(0))
        .expect("zero-reserved layout has an empty valid interval");
    assert_eq!(empty.area().byte_len(), 0);
    assert!(empty.nonempty_range(0, 1).is_none());
}

#[test]
fn page_block_layout_from_page_couples_expected_block_size() {
    let mut blocks = [MaybeUninit::<mi_block_t>::uninit(); 2];
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY: the stack page fixture initializes every geometry field read by
    // PageBlockLayout and its block backing remains live through the checks.
    unsafe {
        let raw = raw_page.as_mut_ptr();
        ptr::addr_of_mut!((*raw).page_start).write(blocks.as_mut_ptr().cast::<u8>());
        ptr::addr_of_mut!((*raw).block_size).write(size_of::<mi_block_t>());
        ptr::addr_of_mut!((*raw).reserved).write(2);
        let page = PagePtr::new_unchecked(raw);
        let expected = BlockSize::new_unchecked(size_of::<mi_block_t>());
        assert!(PageBlockLayout::from_page(page, expected).is_some());

        let mismatched = BlockSize::new_unchecked(2 * size_of::<mi_block_t>());
        assert!(PageBlockLayout::from_page(page, mismatched).is_none());
    }
}

#[test]
fn page_block_layout_rejects_misalignment_and_arithmetic_overflow() {
    let mut words = [0usize; 2];
    // SAFETY: both starts are non-null pointers into live stack storage. The
    // misaligned pointer is rejected before any block pointer is derived.
    let (aligned, misaligned) = unsafe {
        let start = words.as_mut_ptr().cast::<u8>();
        (
            PageStart::new_unchecked(start),
            PageStart::new_unchecked(start.add(1)),
        )
    };
    // SAFETY: the alignment is non-zero.
    let aligned_size = unsafe { BlockSize::new_unchecked(align_of::<mi_block_t>()) };
    assert!(
        PageBlockLayout::from_parts(misaligned, aligned_size, ReservedBlockCount::new(1)).is_none()
    );
    // SAFETY: adding one to the non-zero alignment remains non-zero.
    let misaligned_stride = unsafe { BlockSize::new_unchecked(align_of::<mi_block_t>() + 1) };
    assert!(
        PageBlockLayout::from_parts(aligned, misaligned_stride, ReservedBlockCount::new(1))
            .is_none()
    );

    let maximum_aligned = usize::MAX & !(align_of::<mi_block_t>() - 1);
    // SAFETY: the maximum aligned value is non-zero on supported targets.
    let overflowing_product = unsafe { BlockSize::new_unchecked(maximum_aligned) };
    assert!(
        PageBlockLayout::from_parts(aligned, overflowing_product, ReservedBlockCount::new(2),)
            .is_none()
    );

    let overflowing_end = usize::MAX - ptr_exposed_addr(aligned.as_ptr()) + 1;
    assert!(overflowing_end.is_multiple_of(align_of::<mi_block_t>()));
    // SAFETY: an aligned non-null stack address makes this distance non-zero.
    let overflowing_end = unsafe { BlockSize::new_unchecked(overflowing_end) };
    assert!(
        PageBlockLayout::from_parts(aligned, overflowing_end, ReservedBlockCount::new(1)).is_none()
    );
}
