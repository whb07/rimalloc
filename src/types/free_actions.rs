//! Required types for validating the signatures of free functions.

use super::page::FreePathKind;

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

    #[inline(always)]
    pub(crate) const fn for_free_path(path: FreePathKind) -> Self {
        match path {
            FreePathKind::LocalFast => Self::TrackOnly,
            FreePathKind::LocalSlowFlags
            | FreePathKind::RemotePlain
            | FreePathKind::RemoteSlowFlags => Self::TrackAndCheckFull,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FreePathKind;
    use super::LocalFreeMode;

    #[test]
    fn local_free_mode_names_valid_states() {
        assert!(!LocalFreeMode::TrackOnly.check_full());
        assert!(LocalFreeMode::TrackAndCheckFull.check_full());
        assert_eq!(
            LocalFreeMode::for_free_path(FreePathKind::LocalFast),
            LocalFreeMode::TrackOnly
        );
        assert_eq!(
            LocalFreeMode::for_free_path(FreePathKind::RemoteSlowFlags),
            LocalFreeMode::TrackAndCheckFull
        );
    }
}
