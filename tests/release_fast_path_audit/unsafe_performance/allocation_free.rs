use super::*;

#[path = "allocation_free/marker_counts.rs"]
mod marker_counts;
#[path = "allocation_free/typed_helpers.rs"]
mod typed_helpers;

pub(super) fn assert_allocator_allocation_and_free_boundaries(sources: &UnsafePerformanceSources) {
    marker_counts::assert_allocator_allocation_and_free_boundaries(sources);
}
