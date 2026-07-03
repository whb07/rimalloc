use super::*;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::ptr;

#[test]
fn memkind_code_maps_known_and_unknown_values() {
    assert_eq!(MemKind::from(MemKindCode::NONE), MemKind::None);
    assert_eq!(MemKind::from(MemKindCode::EXTERNAL), MemKind::External);
    assert_eq!(MemKind::from(MemKindCode::STATIC), MemKind::Static);
    assert_eq!(MemKind::from(MemKindCode::META), MemKind::Meta);
    assert_eq!(MemKind::from(MemKindCode::OS), MemKind::Os);
    assert_eq!(MemKind::from(MemKindCode::OS_HUGE), MemKind::OsHuge);
    assert_eq!(MemKind::from(MemKindCode::OS_REMAP), MemKind::OsRemap);
    assert_eq!(MemKind::from(MemKindCode::ARENA), MemKind::Arena);
    assert_eq!(MemKind::from(MemKindCode::HEAP_MAIN), MemKind::HeapMain);
    assert_eq!(
        MemKind::from(MemKindCode(99)),
        MemKind::Unknown(MemKindCode(99))
    );
}

#[test]
fn os_memid_round_trips_through_view() {
    let mut byte = 0u8;
    // SAFETY[AUDIT:typed-memid-test-fixtures]: `byte` is stack-local storage
    // used only as an opaque OS-memory pointer witness for this test.
    let base = unsafe { OsMemPtr::new_unchecked((&mut byte as *mut u8).cast::<c_void>()) };
    let normal = MemId::<OsMem>::os(
        Some(base),
        ReservedBytes::new(2048),
        InitiallyCommitted(false),
        InitiallyZero(true),
        Pinned(false),
    );

    let MemIdView::Os(view) = MemId::<AnyMem>::view(normal.as_raw()) else {
        panic!("expected normal OS memid view");
    };
    assert_eq!(view.base.map(OsMemPtr::as_ptr), Some(base.as_ptr()));
    assert_eq!(view.size.get(), 2048);
    assert!(!view.initially_committed.0);
    assert!(view.initially_zero.0);
    assert!(!view.pinned.0);
    assert!(matches!(view.kind, OsMemKind::Normal));

    let memid = MemId::<OsMem>::os_huge(
        Some(base),
        ReservedBytes::new(4096),
        InitiallyCommitted(true),
        InitiallyZero(false),
        Pinned(true),
    );

    let MemIdView::Os(view) = MemId::<AnyMem>::view(memid.as_raw()) else {
        panic!("expected OS memid view");
    };
    assert_eq!(view.base.map(OsMemPtr::as_ptr), Some(base.as_ptr()));
    assert_eq!(view.size.get(), 4096);
    assert!(view.initially_committed.0);
    assert!(!view.initially_zero.0);
    assert!(view.pinned.0);
    assert!(matches!(view.kind, OsMemKind::Huge));
}

#[test]
fn os_remap_memid_round_trips_through_view() {
    let mut byte = 0u8;
    // SAFETY[AUDIT:typed-memid-test-fixtures]: `byte` is stack-local storage
    // used only as an opaque OS-remap pointer witness for this test.
    let base = unsafe { OsMemPtr::new_unchecked((&mut byte as *mut u8).cast::<c_void>()) };
    let memid = MemId::<OsMem>::os_remap(
        Some(base),
        ReservedBytes::new(16384),
        InitiallyCommitted(false),
        InitiallyZero(true),
        Pinned(false),
    );

    let MemIdView::Os(view) = MemId::<AnyMem>::view(memid.as_raw()) else {
        panic!("expected OS remap memid view");
    };
    assert_eq!(view.base.map(OsMemPtr::as_ptr), Some(base.as_ptr()));
    assert_eq!(view.size.get(), 16384);
    assert!(!view.initially_committed.0);
    assert!(view.initially_zero.0);
    assert!(!view.pinned.0);
    assert!(matches!(view.kind, OsMemKind::Remap));
}

#[test]
fn external_and_meta_memids_round_trip_through_view() {
    let mut byte = 0u8;
    // SAFETY[AUDIT:typed-memid-test-fixtures]: `byte` is stack-local storage
    // used only as an opaque external-memory pointer witness for this test.
    let base = unsafe { OsMemPtr::new_unchecked((&mut byte as *mut u8).cast::<c_void>()) };
    let external = MemId::<ExternalMem>::external(
        Some(base),
        ReservedBytes::new(8192),
        InitiallyCommitted(false),
        InitiallyZero(true),
        Pinned(false),
    );

    let MemIdView::External(view) = MemId::<AnyMem>::view(external.as_raw()) else {
        panic!("expected external memid view");
    };
    assert_eq!(view.base.map(OsMemPtr::as_ptr), Some(base.as_ptr()));
    assert_eq!(view.size.get(), 8192);
    assert!(!view.initially_committed.0);
    assert!(view.initially_zero.0);
    assert!(!view.pinned.0);

    let mut meta_byte = 0u8;
    let meta_page = NonNull::new((&mut meta_byte as *mut u8).cast::<c_void>()).unwrap();
    let meta = MemId::<MetaMem>::meta(
        meta_page,
        BlockIndex(7),
        core::num::NonZeroU32::new(3).unwrap(),
    );

    let MemIdView::Meta(view) = MemId::<AnyMem>::view(meta.as_raw()) else {
        panic!("expected meta memid view");
    };
    assert_eq!(view.meta_page, meta_page);
    assert_eq!(view.block_index.0, 7);
    assert_eq!(view.block_count.get(), 3);
}

#[test]
fn arena_memid_round_trips_slice_metadata() {
    let mut arena = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
    // SAFETY[AUDIT:typed-memid-test-fixtures]: `arena` is stack-local arena
    // metadata, and the pointer remains valid while this test builds views.
    let arena_ptr = unsafe { ArenaPtr::new_unchecked(arena.as_mut_ptr()) };
    // SAFETY[AUDIT:typed-memid-test-fixtures]: the test has unique access to
    // the stack-local arena metadata before constructing the slice range.
    unsafe {
        arena_ptr.set_slice_count(crate::core_types::SliceCount::from_usize(32).unwrap());
    }
    let range = SliceRange::new(11, 5, arena_ptr).unwrap();
    let memid = MemId::<ArenaMem>::arena(
        arena_ptr,
        range,
        InitiallyCommitted(true),
        InitiallyZero(false),
        Pinned(true),
    );

    let MemIdView::Arena(view) = MemId::<AnyMem>::view(memid.as_raw()) else {
        panic!("expected arena memid view");
    };
    assert_eq!(view.arena().as_ptr(), arena_ptr.as_ptr());
    assert_eq!(view.range().start_usize(), 11);
    assert_eq!(view.range().count_usize(), 5);
    assert!(view.initially_committed().0);
    assert!(!view.initially_zero().0);
    assert!(view.pinned().0);
}

#[test]
fn heap_main_memid_round_trips_through_view() {
    let memid = MemId::<HeapMainMem>::heap_main();

    let MemIdView::HeapMain(view) = MemId::<AnyMem>::view(memid.as_raw()) else {
        panic!("expected heap-main memid view");
    };
    assert_eq!(
        view.as_raw().memkind,
        crate::internal::mi_memkind_t::MI_MEM_HEAP_MAIN as _
    );
}

#[test]
fn memid_view_returns_unknown_for_null_meta_or_arena_union_pointers() {
    // SAFETY[AUDIT:typed-memid-test-fixtures]: these raw memids are local
    // invalid-view fixtures that are never published to allocator state.
    let null_meta = crate::internal::mi_memid_t {
        mem: crate::internal::mi_memid_mem_t {
            meta: crate::internal::mi_memid_meta_info_t {
                meta_page: ptr::null_mut(),
                block_index: 0,
                block_count: 1,
            },
        },
        is_pinned: true,
        initially_committed: true,
        initially_zero: true,
        memkind: crate::internal::mi_memkind_t::MI_MEM_META as _,
    };
    let MemIdView::Unknown(code) = MemId::<AnyMem>::view(null_meta) else {
        panic!("expected unknown view for null meta page");
    };
    assert_eq!(code, MemKindCode::META);

    let null_arena = crate::internal::mi_memid_t {
        mem: crate::internal::mi_memid_mem_t {
            arena: crate::internal::mi_memid_arena_info_t {
                arena: ptr::null_mut(),
                slice_index: 0,
                slice_count: 1,
            },
        },
        is_pinned: false,
        initially_committed: false,
        initially_zero: false,
        memkind: crate::internal::mi_memkind_t::MI_MEM_ARENA as _,
    };
    let MemIdView::Unknown(code) = MemId::<AnyMem>::view(null_arena) else {
        panic!("expected unknown view for null arena pointer");
    };
    assert_eq!(code, MemKindCode::ARENA);
}

#[test]
fn memid_view_returns_unknown_for_invalid_arena_slice_range() {
    let mut arena = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
    let arena = arena.as_mut_ptr();

    // SAFETY[AUDIT:typed-memid-test-fixtures]: these raw memids use a
    // stack-local arena pointer only to exercise invalid range rejection.
    let zero_count = crate::internal::mi_memid_t {
        mem: crate::internal::mi_memid_mem_t {
            arena: crate::internal::mi_memid_arena_info_t {
                arena,
                slice_index: 0,
                slice_count: 0,
            },
        },
        is_pinned: false,
        initially_committed: false,
        initially_zero: false,
        memkind: crate::internal::mi_memkind_t::MI_MEM_ARENA as _,
    };
    let MemIdView::Unknown(code) = MemId::<AnyMem>::view(zero_count) else {
        panic!("expected unknown view for zero arena slice count");
    };
    assert_eq!(code, MemKindCode::ARENA);

    let overflow = crate::internal::mi_memid_t {
        mem: crate::internal::mi_memid_mem_t {
            arena: crate::internal::mi_memid_arena_info_t {
                arena,
                slice_index: u32::MAX,
                slice_count: 1,
            },
        },
        is_pinned: false,
        initially_committed: false,
        initially_zero: false,
        memkind: crate::internal::mi_memkind_t::MI_MEM_ARENA as _,
    };
    let MemIdView::Unknown(code) = MemId::<AnyMem>::view(overflow) else {
        panic!("expected unknown view for overflowing arena slice range");
    };
    assert_eq!(code, MemKindCode::ARENA);
}
