use super::*;

#[test]
fn internal_layout_error_is_distinct_from_public_safe_error() {
    assert_ne!(
        core::any::TypeId::of::<LayoutError>(),
        core::any::TypeId::of::<crate::safe::LayoutError>()
    );
}

#[test]
fn malloc_layout_preserves_requested_and_normalizes_effective_size() {
    let zero = LayoutRequest::malloc(0).expect("zero-size malloc layout");
    assert_eq!(zero.requested().get(), 0);
    assert_eq!(zero.effective().get(), RequestedSize::MIN_ZERO_SIZE);
    assert_eq!(
        zero.padded().get(),
        crate::types::MI_PADDING_SIZE + RequestedSize::MIN_ZERO_SIZE
    );
    assert_eq!(zero.zeroed(), Zeroed::No);

    let nonzero = LayoutRequest::malloc(17).expect("malloc layout");
    assert_eq!(nonzero.requested().get(), 17);
    assert_eq!(nonzero.effective().get(), 17);
    assert!(nonzero.padded().get() >= 17);
}

#[test]
fn layout_request_from_parts_is_the_checked_padding_path() {
    let req = LayoutRequest::from_parts(
        RequestedSize::new(0),
        unsafe { EffectiveSize::new_unchecked(1) },
        unsafe { MaybeAlignment::new_unchecked(0) },
        Zeroed::No,
    )
    .expect("layout from parts");
    assert_eq!(req.padded().get(), crate::types::MI_PADDING_SIZE + 1);
}

#[test]
fn zeroing_and_alignment_flags_are_explicit() {
    assert_eq!(core::mem::size_of::<Zeroed>(), core::mem::size_of::<bool>());
    assert_eq!(Zeroed::from_bool(false), Zeroed::No);
    assert_eq!(Zeroed::from_bool(true), Zeroed::Yes);
    assert!(!Zeroed::No.as_bool());
    assert!(Zeroed::Yes.as_bool());

    let zalloc = LayoutRequest::zalloc(8).expect("zalloc layout");
    assert_eq!(zalloc.zeroed(), Zeroed::Yes);

    let aligned = LayoutRequest::aligned(8, 64).expect("aligned layout");
    assert_eq!(aligned.alignment().get(), 64);

    assert_eq!(
        LayoutRequest::aligned(8, 24).err(),
        Some(LayoutError::AlignmentInvalid)
    );
}

#[test]
fn calloc_request_checks_total_zeroing_and_overflow() {
    let req = CallocRequest::new(4, 32).expect("calloc request");
    assert_eq!(req.total().get(), 128);
    assert_eq!(req.layout().zeroed(), Zeroed::Yes);

    assert_eq!(
        CallocRequest::new(usize::MAX, 2).err(),
        Some(LayoutError::SizeOverflow)
    );
}

#[test]
fn realloc_request_constructors_distinguish_null_from_existing() {
    let layout = LayoutRequest::malloc(64).expect("realloc layout");
    let null = ReallocRequest::from_null(layout);
    assert!(null.ptr().is_none());
    assert!(null.old().is_none());

    let mut raw_page = core::mem::MaybeUninit::<crate::types::mi_page_t>::zeroed();
    let mut storage = [0usize; 128 / core::mem::size_of::<usize>() + 2];
    let block = crate::core_types::BlockPtr::new(storage.as_mut_ptr().cast()).unwrap();
    let page = PagePtr::new(raw_page.as_mut_ptr()).unwrap();
    // SAFETY: the stack fixtures provide aligned, initialized page geometry and
    // block/padding storage for the complete lookup and consuming request.
    let (user, old) = unsafe {
        (*page.as_ptr()).block_size = 128 + crate::types::MI_PADDING_SIZE;
        (*page.as_ptr()).page_start = block.as_ptr().cast();
        let keys = crate::core_types::PageKeys::disabled();
        page.set_keys(keys);
        crate::core_types::PaddingPtr::from_block_usable(block, UsableSize::new(128)).write(
            crate::core_types::Padding::new(
                crate::core_types::PaddingCanary::from_page_block_keys(page, block, keys),
                crate::core_types::PaddingDelta::for_usable(0, UsableSize::new(128)).unwrap(),
            ),
        );
        let user = UserPtr::new(block.as_ptr().cast::<u8>().add(8).cast()).unwrap();
        let lookup = crate::core_types::PointerLookup::after_validation(
            user,
            page,
            crate::core_types::PointerLocation::Interior {
                block,
                offset: crate::core_types::BlockOffset(8),
            },
        );
        let crate::core_types::PointerLookup::Valid(valid) = lookup else {
            unreachable!()
        };
        let old = OldAllocation::from_validated(valid);
        crate::allocator::tracking::mi_track_mem_defined(
            block.as_ptr().cast(),
            core::mem::size_of_val(&storage),
        );
        (user, old)
    };
    let existing = ReallocRequest::from_existing(old, layout);
    assert_eq!(existing.ptr().map(|ptr| ptr.as_ptr()), Some(user.as_ptr()));
    assert_eq!(existing.old().unwrap().usable().get(), 120);
    let (old, new_layout) = existing.into_parts();
    assert_eq!(new_layout.requested().get(), 64);
    assert_eq!(old.unwrap().into_ptr(), user);
}

#[test]
fn preserve_bytes_is_min_of_old_usable_and_new_requested() {
    let shrink = PreserveBytes::new(UsableSize::new(128), RequestedSize::new(64));
    assert_eq!(shrink.get(), 64);

    let grow = PreserveBytes::new(UsableSize::new(128), RequestedSize::new(192));
    assert_eq!(grow.get(), 128);
}

#[test]
fn generic_flags_pack_zero_bit_and_huge_alignment() {
    let alignment = Alignment::new(crate::types::MI_PAGE_MAX_OVERALLOC_ALIGN * 2).unwrap();
    let huge = MaybeHugeAlignment::for_overalloc(alignment).unwrap();
    let flags = MallocGenericFlags::new(Zeroed::Yes, huge);
    assert_eq!(flags.zeroed(), Zeroed::Yes);
    assert_eq!(flags.raw_huge_alignment(), alignment.get());

    let raw = MallocGenericFlags::from_raw(128);
    assert_eq!(raw.zeroed(), Zeroed::No);
    assert_eq!(raw.raw_huge_alignment(), 128);

    let raw_zero_huge = MallocGenericFlags::from_raw(65);
    assert_eq!(raw_zero_huge.zeroed(), Zeroed::Yes);
    assert_eq!(raw_zero_huge.raw_huge_alignment(), 64);
}

#[test]
fn huge_alignment_requires_the_existing_large_alignment_boundary() {
    for shift in 0..usize::BITS {
        let alignment = Alignment::new(1usize << shift).unwrap();
        let huge = MaybeHugeAlignment::for_overalloc(alignment);
        assert_eq!(
            huge.is_some(),
            alignment.get() > crate::types::MI_PAGE_MAX_OVERALLOC_ALIGN
        );
        for zeroed in [Zeroed::No, Zeroed::Yes] {
            let payload = huge.unwrap_or_else(MaybeHugeAlignment::none);
            let flags = MallocGenericFlags::new(zeroed, payload);
            assert_eq!(flags.zeroed(), zeroed);
            assert_eq!(flags.raw_huge_alignment(), payload.get());
        }
    }
}
