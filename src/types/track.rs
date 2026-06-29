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
mod tests;
