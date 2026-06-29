use core::ffi::c_long;
use core::fmt;

/// Error returned by safe options APIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OptionError {
    /// The raw option code is outside mimalloc's option range.
    InvalidKey,
    /// The option exists in raw mimalloc but is not supported or mutable here.
    Unsupported,
}

impl fmt::Display for OptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => f.write_str("invalid mimalloc option key"),
            Self::Unsupported => f.write_str("unsupported mimalloc option operation"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OptionError {}

/// Safe mimalloc option key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionKey {
    /// Controls whether mimalloc prints error diagnostics.
    ShowErrors,
    /// Controls whether mimalloc prints statistics at process exit.
    ShowStats,
    /// Maximum number of error diagnostics.
    MaxErrors,
    /// Maximum number of warning diagnostics.
    MaxWarnings,
}

impl OptionKey {
    /// All option keys currently supported by the safe facade.
    pub const ALL: [Self; 4] = [
        Self::ShowErrors,
        Self::ShowStats,
        Self::MaxErrors,
        Self::MaxWarnings,
    ];

    /// Converts a raw mimalloc option code into a supported safe option key.
    #[inline]
    pub fn from_raw(raw: i32) -> Result<Self, OptionError> {
        match raw {
            0 => Ok(Self::ShowErrors),
            1 => Ok(Self::ShowStats),
            19 => Ok(Self::MaxErrors),
            20 => Ok(Self::MaxWarnings),
            raw if (0..crate::options::mi_option_t::_mi_option_last.as_i32()).contains(&raw) => {
                Err(OptionError::Unsupported)
            }
            _ => Err(OptionError::InvalidKey),
        }
    }

    /// Returns the raw mimalloc option code for this key.
    #[inline]
    #[must_use]
    pub const fn as_raw(self) -> i32 {
        self.raw_option().as_i32()
    }

    #[inline]
    pub(super) const fn raw_option(self) -> crate::options::mi_option_t {
        match self {
            Self::ShowErrors => crate::options::mi_option_t::mi_option_show_errors,
            Self::ShowStats => crate::options::mi_option_t::mi_option_show_stats,
            Self::MaxErrors => crate::options::mi_option_t::mi_option_max_errors,
            Self::MaxWarnings => crate::options::mi_option_t::mi_option_max_warnings,
        }
    }

    #[inline]
    const fn mutable(self) -> bool {
        !matches!(self, Self::ShowStats)
    }

    #[inline]
    pub(super) const fn default_value(self) -> c_long {
        match self {
            Self::ShowErrors => {
                if cfg!(debug_assertions) {
                    1
                } else {
                    0
                }
            }
            Self::ShowStats => 0,
            Self::MaxErrors => 32,
            Self::MaxWarnings => 32,
        }
    }
}

impl TryFrom<i32> for OptionKey {
    type Error = OptionError;

    #[inline]
    fn try_from(raw: i32) -> Result<Self, Self::Error> {
        Self::from_raw(raw)
    }
}

/// Safe access to supported mimalloc options.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options;

impl Options {
    /// Returns the current value for a supported option key.
    #[inline]
    #[must_use]
    pub fn get(key: OptionKey) -> c_long {
        crate::options::mi_option_get(key.raw_option())
    }

    /// Sets a mutable supported option key.
    #[inline]
    pub fn set(key: OptionKey, value: c_long) -> Result<(), OptionError> {
        if !key.mutable() {
            return Err(OptionError::Unsupported);
        }
        crate::options::mi_option_set(key.raw_option(), value);
        Ok(())
    }

    /// Resets a mutable supported option key to its default value.
    #[inline]
    pub fn reset(key: OptionKey) -> Result<(), OptionError> {
        Self::set(key, key.default_value())
    }
}
