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

    let user = unsafe { UserPtr::new_unchecked(core::ptr::dangling_mut::<u8>().cast()) };
    let page = unsafe { PagePtr::new_unchecked(core::ptr::dangling_mut()) };
    let old = unsafe { OldAllocation::from_validated_parts(user, UsableSize::new(128), page) };
    let existing = ReallocRequest::from_existing(old, layout);
    assert_eq!(existing.ptr().map(|ptr| ptr.as_ptr()), Some(user.as_ptr()));
    assert!(existing.old().is_some());
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
    let flags = MallocGenericFlags::new(Zeroed::Yes, MaybeHugeAlignment::new(64));
    assert_eq!(flags.zeroed(), Zeroed::Yes);
    assert_eq!(flags.huge_alignment().get(), 64);

    let raw = MallocGenericFlags::from_raw(128);
    assert_eq!(raw.zeroed(), Zeroed::No);
    assert_eq!(raw.huge_alignment().get(), 128);

    let raw_zero_huge = MallocGenericFlags::from_raw(65);
    assert_eq!(raw_zero_huge.zeroed(), Zeroed::Yes);
    assert_eq!(raw_zero_huge.huge_alignment().get(), 64);
}
