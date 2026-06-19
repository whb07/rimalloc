//! Tracking backend mode types.

/// Compile-time selected allocation tracking backend.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TrackMode {
    Disabled,
    Valgrind,
    Asan,
}

impl TrackMode {
    pub(crate) const CURRENT: Self = if cfg!(feature = "track-valgrind") {
        Self::Valgrind
    } else if cfg!(feature = "track-asan") {
        Self::Asan
    } else {
        Self::Disabled
    };

    #[inline(always)]
    pub(crate) const fn is_enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    #[inline(always)]
    pub(crate) const fn valgrind_flag(self) -> usize {
        match self {
            Self::Valgrind => 1,
            Self::Disabled | Self::Asan => 0,
        }
    }

    #[inline(always)]
    pub(crate) const fn asan_flag(self) -> usize {
        match self {
            Self::Asan => 1,
            Self::Disabled | Self::Valgrind => 0,
        }
    }

    #[inline(always)]
    pub(crate) const fn enabled_flag(self) -> usize {
        self.is_enabled() as usize
    }

    #[inline(always)]
    pub(crate) const fn heap_destroy_flag(self) -> usize {
        match self {
            Self::Valgrind => 1,
            Self::Disabled | Self::Asan => 0,
        }
    }

    #[inline(always)]
    pub(crate) const fn tool_name(self) -> &'static str {
        match self {
            Self::Disabled => "none",
            Self::Valgrind => "valgrind",
            Self::Asan => "asan",
        }
    }
}

#[cfg(test)]
mod tests {
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
}
