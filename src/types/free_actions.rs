//! Required types for validating the signatures of free functions.

/// Local free mode options.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LocalFreeMode {
    /// Track stats and unfull pages when needed.
    TrackAndCheckFull,
    /// Track stats but skip the full-queue check.
    TrackOnly,
}

impl LocalFreeMode {
    #[inline(always)]
    pub(crate) const fn check_full(self) -> bool {
        matches!(self, Self::TrackAndCheckFull)
    }
}

#[cfg(test)]
mod tests {
    use super::LocalFreeMode;

    #[test]
    fn local_free_mode_names_valid_states() {
        assert!(!LocalFreeMode::TrackOnly.check_full());
        assert!(LocalFreeMode::TrackAndCheckFull.check_full());
    }
}
