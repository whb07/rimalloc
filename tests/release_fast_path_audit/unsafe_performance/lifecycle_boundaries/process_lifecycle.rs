use super::*;

#[path = "process_lifecycle/heap_boundaries.rs"]
mod heap_boundaries;
#[path = "process_lifecycle/init_markers.rs"]
mod init_markers;
#[path = "process_lifecycle/subproc_fast_paths.rs"]
mod subproc_fast_paths;

pub(super) fn assert_process_lifecycle_boundaries(sources: &UnsafePerformanceSources) {
    heap_boundaries::assert_process_lifecycle_boundaries(sources);
}
