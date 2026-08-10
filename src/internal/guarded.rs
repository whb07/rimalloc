//! Guarded allocation configuration types.

/// Minimum guarded allocation request size.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct GuardedSizeMin(usize);

impl GuardedSizeMin {
    #[inline(always)]
    pub(crate) const fn new(size: usize) -> Self {
        Self(size)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Maximum guarded allocation request size.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct GuardedSizeMax(usize);

impl GuardedSizeMax {
    #[inline(always)]
    pub(crate) const fn new(size: usize) -> Self {
        Self(size)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Guarded allocation sample rate.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct GuardedSampleRate(usize);

impl GuardedSampleRate {
    #[inline(always)]
    pub(crate) const fn new(rate: usize) -> Self {
        Self(rate)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Guarded allocation countdown until the next sample attempt.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct GuardedSampleCount(usize);

impl GuardedSampleCount {
    #[inline(always)]
    pub(crate) const fn new(count: usize) -> Self {
        Self(count)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn decrement_wrapping(self) -> Self {
        Self(self.0.wrapping_sub(1))
    }

    #[inline(always)]
    pub(crate) const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Typed view of guarded allocation sampling configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct GuardedConfig {
    size_min: GuardedSizeMin,
    size_max: GuardedSizeMax,
    sample_rate: GuardedSampleRate,
    sample_count: GuardedSampleCount,
}

impl GuardedConfig {
    #[inline(always)]
    pub(crate) const fn new(
        size_min: GuardedSizeMin,
        size_max: GuardedSizeMax,
        sample_rate: GuardedSampleRate,
        sample_count: GuardedSampleCount,
    ) -> Option<Self> {
        if size_min.get() <= size_max.get() {
            Some(Self {
                size_min,
                size_max,
                sample_rate,
                sample_count,
            })
        } else {
            None
        }
    }

    /// # Safety
    /// `size_min <= size_max` must hold.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(
        size_min: GuardedSizeMin,
        size_max: GuardedSizeMax,
        sample_rate: GuardedSampleRate,
        sample_count: GuardedSampleCount,
    ) -> Self {
        Self {
            size_min,
            size_max,
            sample_rate,
            sample_count,
        }
    }

    #[inline(always)]
    pub(crate) const fn from_clamped_raw(
        size_min: usize,
        size_max: usize,
        sample_rate: usize,
        sample_count: usize,
    ) -> Self {
        let size_min = GuardedSizeMin::new(size_min);
        let size_max = if size_max < size_min.get() {
            GuardedSizeMax::new(size_min.get())
        } else {
            GuardedSizeMax::new(size_max)
        };
        // SAFETY: guarded initialization clamps
        // `size_max` to at least `size_min` before constructing the private
        // typed configuration.
        unsafe {
            Self::new_unchecked(
                size_min,
                size_max,
                GuardedSampleRate::new(sample_rate),
                GuardedSampleCount::new(sample_count),
            )
        }
    }

    #[inline(always)]
    pub(crate) const fn size_min(self) -> GuardedSizeMin {
        self.size_min
    }

    #[inline(always)]
    pub(crate) const fn size_max(self) -> GuardedSizeMax {
        self.size_max
    }

    #[inline(always)]
    pub(crate) const fn sample_rate(self) -> GuardedSampleRate {
        self.sample_rate
    }

    #[inline(always)]
    pub(crate) const fn sample_count(self) -> GuardedSampleCount {
        self.sample_count
    }

    #[inline(always)]
    pub(crate) const fn contains_size(self, size: usize) -> bool {
        size >= self.size_min.get() && size <= self.size_max.get()
    }

    #[inline(always)]
    pub(crate) const fn decide(self, size: usize) -> GuardedDecision {
        let count = self.sample_count.decrement_wrapping();
        if !count.is_zero() {
            GuardedDecision::SkipAndUpdate {
                next_sample_count: count,
            }
        } else if self.contains_size(size) && !self.sample_rate.is_zero() {
            GuardedDecision::UseGuarded {
                next_sample_count: GuardedSampleCount::new(self.sample_rate.get()),
            }
        } else if self.sample_rate.is_zero() {
            GuardedDecision::SkipUnchanged
        } else {
            GuardedDecision::SkipAndUpdate {
                next_sample_count: GuardedSampleCount::new(1),
            }
        }
    }
}

/// Result of guarded allocation sampling for one allocation request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GuardedDecision {
    UseGuarded {
        next_sample_count: GuardedSampleCount,
    },
    SkipAndUpdate {
        next_sample_count: GuardedSampleCount,
    },
    SkipUnchanged,
}

impl GuardedDecision {
    #[inline(always)]
    pub(crate) const fn next_sample_count(self) -> Option<GuardedSampleCount> {
        match self {
            Self::UseGuarded { next_sample_count } | Self::SkipAndUpdate { next_sample_count } => {
                Some(next_sample_count)
            }
            Self::SkipUnchanged => None,
        }
    }

    #[inline(always)]
    pub(crate) const fn is_guarded(self) -> bool {
        matches!(self, Self::UseGuarded { .. })
    }
}

#[cfg(test)]
mod tests;
