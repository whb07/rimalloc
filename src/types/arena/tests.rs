use super::*;
use core::mem::MaybeUninit;

fn with_arena_slice_count<R>(count: usize, f: impl FnOnce(ArenaPtr) -> R) -> R {
    let mut raw = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
    // SAFETY[AUDIT:typed-arena-test-fixtures]: `raw` is stack-local arena
    // metadata, and the pointer remains valid for the duration of `f`.
    let arena = unsafe { ArenaPtr::new_unchecked(raw.as_mut_ptr()) };
    // SAFETY[AUDIT:typed-arena-test-fixtures]: the test has unique access to
    // the stack-local arena metadata before passing the typed pointer to `f`.
    unsafe {
        arena.set_slice_count(SliceCount::from_usize(count).unwrap());
    }
    f(arena)
}

fn with_arena_state<R>(
    is_exclusive: bool,
    parent: Option<ArenaPtr>,
    f: impl FnOnce(ArenaPtr) -> R,
) -> R {
    let mut raw = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
    // SAFETY[AUDIT:typed-arena-test-fixtures]: `raw` is stack-local arena
    // metadata, and the pointer remains valid for the duration of `f`.
    let arena = unsafe { ArenaPtr::new_unchecked(raw.as_mut_ptr()) };
    // SAFETY[AUDIT:typed-arena-test-fixtures]: the test has unique access to
    // this stack-local arena metadata while initializing role fields.
    unsafe {
        arena.set_is_exclusive(is_exclusive);
        arena.set_parent(parent);
    }
    f(arena)
}

#[test]
fn slice_range_new_accepts_range_ending_at_arena_slice_count() {
    let range = with_arena_slice_count(16, |arena| SliceRange::new(12, 4, arena).unwrap());

    assert_eq!(range.start_usize(), 12);
    assert_eq!(range.start_u32(), 12);
    assert_eq!(range.count_usize(), 4);
    assert_eq!(range.count_u32(), 4);
    assert_eq!(range.end_usize(), 16);
}

#[test]
fn slice_range_new_rejects_range_past_arena_slice_count() {
    let err = with_arena_slice_count(16, |arena| SliceRange::new(13, 4, arena).unwrap_err());

    assert_eq!(err, SliceError::OutOfBounds);
}

#[test]
fn slice_range_new_rejects_overflowing_range_end() {
    let err = with_arena_slice_count(u32::MAX as usize, |arena| {
        SliceRange::new(u32::MAX as usize, 1, arena).unwrap_err()
    });

    assert_eq!(err, SliceError::Overflow);
}

#[test]
fn slice_range_checked_offset_rejects_overflowing_addition() {
    let range = SliceRange::from_raw(u32::MAX - 1, 1).unwrap();

    let err = range.checked_add_offset(2).unwrap_err();

    assert_eq!(err, SliceError::Overflow);
}

#[test]
fn slice_range_single_at_offset_uses_checked_range_bounds() {
    let arena = with_arena_slice_count(16, |arena| SliceRange::new(12, 4, arena).unwrap());

    let single = arena.single_at_offset(3).unwrap();
    let err = arena.single_at_offset(4).unwrap_err();

    assert_eq!(single.start_usize(), 15);
    assert_eq!(single.count_usize(), 1);
    assert_eq!(err, SliceError::OutOfBounds);
}

#[test]
fn arena_role_constructors_follow_raw_exclusive_state() {
    with_arena_state(false, None, |arena| {
        assert_eq!(
            // SAFETY[AUDIT:typed-arena-test-fixtures]: `with_arena_state`
            // initialized stack-local metadata as a non-exclusive root arena.
            unsafe { SharedArena::new(arena).unwrap().ptr().as_ptr() },
            arena.as_ptr()
        );
        // SAFETY[AUDIT:typed-arena-test-fixtures]: the same fixture is marked
        // non-exclusive, so the exclusive constructor must reject it.
        assert!(unsafe { ExclusiveArena::new(arena).is_none() });
    });

    with_arena_state(true, None, |arena| {
        assert_eq!(
            // SAFETY[AUDIT:typed-arena-test-fixtures]: `with_arena_state`
            // initialized stack-local metadata as an exclusive root arena.
            unsafe { ExclusiveArena::new(arena).unwrap().ptr().as_ptr() },
            arena.as_ptr()
        );
        // SAFETY[AUDIT:typed-arena-test-fixtures]: the same fixture is marked
        // exclusive, so the shared constructor must reject it.
        assert!(unsafe { SharedArena::new(arena).is_none() });
    });
}

#[test]
fn parent_arena_constructor_rejects_child_arena_state() {
    let mut raw_parent = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
    // SAFETY[AUDIT:typed-arena-test-fixtures]: `raw_parent` is stack-local
    // arena metadata used only as an opaque parent identity in this test.
    let parent = unsafe { ArenaPtr::new_unchecked(raw_parent.as_mut_ptr()) };

    with_arena_state(false, None, |arena| {
        assert_eq!(
            // SAFETY[AUDIT:typed-arena-test-fixtures]: the fixture is a
            // non-exclusive arena with no parent, matching `ParentArena`.
            unsafe { ParentArena::new(arena).unwrap().ptr().as_ptr() },
            arena.as_ptr()
        );
    });

    with_arena_state(false, Some(parent), |arena| {
        // SAFETY[AUDIT:typed-arena-test-fixtures]: the fixture has a parent
        // pointer, so the parent-arena constructor must reject it.
        assert!(unsafe { ParentArena::new(arena).is_none() });
    });
}
