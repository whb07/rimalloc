use mechanical_mimalloc::{internal, track};

#[test]
fn internal_tracking_constants_follow_track_module() {
    assert_eq!(internal::MI_TRACK_ASAN, track::MI_TRACK_ASAN);
    assert_eq!(internal::MI_TRACK_ENABLED, track::MI_TRACK_ENABLED);
}
