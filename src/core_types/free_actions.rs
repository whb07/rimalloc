//! Required types for validating the signatures of free functions.

/// Local free mode options.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct LocalFreeMode {
    pub(crate) track_stats: bool,
    pub(crate) check_full: bool,
}
