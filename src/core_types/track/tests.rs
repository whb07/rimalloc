
use super::*;

#[test]
fn current_mode_matches_enabled_features() {
    #[cfg(feature = "track-valgrind")]
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
