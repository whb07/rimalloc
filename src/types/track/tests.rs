use super::*;

#[test]
fn current_mode_matches_enabled_features() {
    #[cfg(all(feature = "track-valgrind", feature = "track-asan"))]
    assert_eq!(TrackMode::CURRENT, TrackMode::Valgrind);

    #[cfg(all(feature = "track-valgrind", not(feature = "track-asan")))]
    assert_eq!(TrackMode::CURRENT, TrackMode::Valgrind);

    #[cfg(all(not(feature = "track-valgrind"), feature = "track-asan"))]
    assert_eq!(TrackMode::CURRENT, TrackMode::Asan);

    #[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
    assert_eq!(TrackMode::CURRENT, TrackMode::Disabled);
}

#[test]
fn flags_match_modes() {
    assert_eq!(TrackMode::Disabled.enabled_flag(), 0);
    assert_eq!(TrackMode::Disabled.valgrind_flag(), 0);
    assert_eq!(TrackMode::Disabled.asan_flag(), 0);
    assert_eq!(TrackMode::Disabled.heap_destroy_flag(), 0);
    assert_eq!(TrackMode::Disabled.tool_name(), "none");

    assert_eq!(TrackMode::Valgrind.enabled_flag(), 1);
    assert_eq!(TrackMode::Valgrind.valgrind_flag(), 1);
    assert_eq!(TrackMode::Valgrind.asan_flag(), 0);
    assert_eq!(TrackMode::Valgrind.heap_destroy_flag(), 1);
    assert_eq!(TrackMode::Valgrind.tool_name(), "valgrind");

    assert_eq!(TrackMode::Asan.enabled_flag(), 1);
    assert_eq!(TrackMode::Asan.valgrind_flag(), 0);
    assert_eq!(TrackMode::Asan.asan_flag(), 1);
    assert_eq!(TrackMode::Asan.heap_destroy_flag(), 0);
    assert_eq!(TrackMode::Asan.tool_name(), "asan");
}

#[test]
fn internal_tracking_constants_follow_track_module() {
    assert_eq!(
        crate::internal::MI_TRACK_ASAN,
        crate::allocator::tracking::MI_TRACK_ASAN
    );
    assert_eq!(
        crate::internal::MI_TRACK_ENABLED,
        crate::allocator::tracking::MI_TRACK_ENABLED
    );
}

#[cfg(not(any(feature = "track-valgrind", feature = "track-asan")))]
#[test]
fn default_tracking_mode_is_disabled() {
    assert_eq!(crate::allocator::tracking::MI_TRACK_ENABLED, 0);
    assert_eq!(crate::allocator::tracking::MI_TRACK_HEAP_DESTROY, 0);
    assert_eq!(crate::allocator::tracking::MI_TRACK_TOOL, "none");
}

#[cfg(all(feature = "track-valgrind", feature = "track-asan"))]
#[test]
fn aggregate_tracking_features_prefer_valgrind() {
    assert_eq!(crate::allocator::tracking::MI_TRACK_ENABLED, 1);
    assert_eq!(crate::allocator::tracking::MI_TRACK_VALGRIND, 1);
    assert_eq!(crate::allocator::tracking::MI_TRACK_ASAN, 0);
    assert_eq!(crate::allocator::tracking::MI_TRACK_TOOL, "valgrind");
}
