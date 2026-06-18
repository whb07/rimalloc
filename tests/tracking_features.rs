use mechanical_mimalloc::raw::{internal, track};

#[test]
fn internal_tracking_constants_follow_track_module() {
    assert_eq!(internal::MI_TRACK_ASAN, track::MI_TRACK_ASAN);
    assert_eq!(internal::MI_TRACK_ENABLED, track::MI_TRACK_ENABLED);
}

#[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
#[test]
fn default_tracking_mode_is_disabled() {
    assert_eq!(track::MI_TRACK_ENABLED, 0);
    assert_eq!(track::MI_TRACK_HEAP_DESTROY, 0);
    assert_eq!(track::MI_TRACK_TOOL, "none");
}
