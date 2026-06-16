//! Required types for validating the signatures of free functions.

use crate::core_types::{
    BlockForFree, ForeignPage, LocalPage, OwnedAbandonedPage, ThreadHeapLocal,
    ActivePage, ReclaimResult,
};

/// Local free mode options.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct LocalFreeMode {
    pub(crate) track_stats: bool,
    pub(crate) check_full: bool,
}

// These empty signatures enforce the strict type boundaries defined in the PRD (DT-9).
// Eventually the actual free logic from `free.rs` will be updated to conform to these.

#[inline]
pub(crate) fn free_block_local(
    _page: LocalPage,
    _block: BlockForFree,
    _mode: LocalFreeMode,
) {
    unimplemented!()
}

#[inline]
pub(crate) fn free_block_remote(
    _page: ForeignPage,
    _block: BlockForFree,
) {
    unimplemented!()
}

#[inline]
pub(crate) fn try_reclaim_abandoned(
    _page: OwnedAbandonedPage,
    _target: ThreadHeapLocal,
) -> ReclaimResult {
    unimplemented!()
}