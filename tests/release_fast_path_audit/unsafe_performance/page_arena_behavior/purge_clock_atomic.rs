use super::PageArenaBehaviorSources;

#[path = "purge_clock_atomic/arena_count_atomics.rs"]
mod arena_count_atomics;
#[path = "purge_clock_atomic/arena_page_atomics.rs"]
mod arena_page_atomics;
#[path = "purge_clock_atomic/clock_helpers.rs"]
mod clock_helpers;

pub(super) fn assert_purge_clock_and_atomic_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    clock_helpers::assert_clock_helper_boundaries(sources);
    arena_count_atomics::assert_arena_count_atomic_boundaries(sources);
    arena_page_atomics::assert_arena_page_atomic_boundaries(sources);
}
