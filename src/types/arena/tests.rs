use super::*;
use core::mem::MaybeUninit;

fn with_arena_slice_count<R>(count: usize, f: impl FnOnce(ArenaPtr) -> R) -> R {
    let mut raw = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
    // SAFETY: `raw` is stack-local arena
    // metadata, and the pointer remains valid for the duration of `f`.
    let arena = unsafe { ArenaPtr::new_unchecked(raw.as_mut_ptr()) };
    // SAFETY: the test has unique access to
    // the stack-local arena metadata before passing the typed pointer to `f`.
    unsafe { (*arena.as_ptr()).slice_count = count }
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
