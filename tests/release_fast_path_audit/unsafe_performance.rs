use super::*;
#[path = "unsafe_performance/allocation_free.rs"]
mod allocation_free;
#[path = "unsafe_performance/arena_boundaries.rs"]
mod arena_boundaries;
#[path = "unsafe_performance/bitmap_boundaries.rs"]
mod bitmap_boundaries;
#[path = "unsafe_performance/heap_page_visitors.rs"]
mod heap_page_visitors;
#[path = "unsafe_performance/lifecycle_boundaries.rs"]
mod lifecycle_boundaries;
#[path = "unsafe_performance/local_boundaries.rs"]
mod local_boundaries;
#[path = "unsafe_performance/os_memory.rs"]
mod os_memory;
#[path = "unsafe_performance/page_arena_behavior.rs"]
mod page_arena_behavior;
#[path = "unsafe_performance/page_lifecycle.rs"]
mod page_lifecycle;
#[path = "unsafe_performance/platform_primitive_boundaries.rs"]
mod platform_primitive_boundaries;
#[path = "unsafe_performance/raw_facade.rs"]
mod raw_facade;
#[path = "unsafe_performance/shared.rs"]
mod shared;
#[path = "unsafe_performance/size_class_boundaries.rs"]
mod size_class_boundaries;
#[path = "unsafe_performance/tail_guards.rs"]
mod tail_guards;
#[path = "unsafe_performance/thread_stats.rs"]
mod thread_stats;
#[path = "unsafe_performance/threadlocal_boundaries.rs"]
mod threadlocal_boundaries;

use shared::{UnsafePerformanceSources, source_has_inline_never_function};

#[test]
fn free_path_internals_are_not_dynamic_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = UnsafePerformanceSources::load(root);

    allocation_free::assert_allocator_allocation_and_free_boundaries(&sources);
    raw_facade::assert_raw_facade_boundaries(&sources);
    os_memory::assert_os_memory_and_platform_boundaries(&sources);
    page_arena_behavior::assert_page_arena_behavior_boundaries(&sources);
    thread_stats::assert_thread_heap_and_stats_boundaries(&sources);
}
