use super::*;

#[test]
fn slice_range_new_accepts_range_ending_at_arena_slice_count() {
    let range = SliceRange::new(12, 4, SliceCount::from_usize(16).unwrap()).unwrap();

    assert_eq!(range.start_usize(), 12);
    assert_eq!(range.start_u32(), 12);
    assert_eq!(range.count_usize(), 4);
    assert_eq!(range.count_u32(), 4);
    assert_eq!(range.end_usize(), 16);
}

#[test]
fn slice_range_new_rejects_range_past_arena_slice_count() {
    let err = SliceRange::new(13, 4, SliceCount::from_usize(16).unwrap()).unwrap_err();

    assert_eq!(err, SliceError::OutOfBounds);
}

#[test]
fn slice_range_new_rejects_overflowing_range_end() {
    let extent = SliceCount::from_usize(u32::MAX as usize).unwrap();
    let err = SliceRange::new(u32::MAX as usize, 1, extent).unwrap_err();

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
    let arena = SliceRange::new(12, 4, SliceCount::from_usize(16).unwrap()).unwrap();

    let single = arena.single_at_offset(3).unwrap();
    let err = arena.single_at_offset(4).unwrap_err();

    assert_eq!(single.start_usize(), 15);
    assert_eq!(single.count_usize(), 1);
    assert_eq!(err, SliceError::OutOfBounds);
}

#[test]
fn slice_range_rejects_empty_and_unrepresentable_counts() {
    let extent = SliceCount::from_usize(16).unwrap();
    assert_eq!(
        SliceRange::new(0, 0, extent).unwrap_err(),
        SliceError::Overflow
    );
    assert_eq!(
        SliceRange::new(0, usize::MAX, extent).unwrap_err(),
        if usize::BITS > 32 {
            SliceError::Overflow
        } else {
            SliceError::OutOfBounds
        },
    );
}
