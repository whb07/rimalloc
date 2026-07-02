use super::*;

#[path = "raw_override/allocator_page.rs"]
mod allocator_page;
#[path = "raw_override/internal_imports.rs"]
mod internal_imports;
#[path = "raw_override/libc_helpers.rs"]
mod libc_helpers;
#[path = "raw_override/options_stats.rs"]
mod options_stats;
#[path = "raw_override/os_random.rs"]
mod os_random;
#[path = "raw_override/runtime_heap.rs"]
mod runtime_heap;

pub(super) fn assert_raw_override_boundaries(sources: &UnsafePerformanceSources) {
    allocator_page::assert_allocator_page_no_exports(sources);
    libc_helpers::assert_libc_helper_no_exports(sources);
    os_random::assert_os_random_no_exports(sources);
    runtime_heap::assert_runtime_heap_no_exports(sources);
    options_stats::assert_options_stats_no_exports(sources);
    internal_imports::assert_internal_import_hygiene(sources);
}

fn assert_internal_no_export(source_name: &str, source: &str, internal_name: &str) {
    let no_mangle_export = format!("#[unsafe(no_mangle)]\n{internal_name}");
    assert!(
        !source.contains(&no_mangle_export),
        "{internal_name} in {source_name} should stay crate-internal instead of a dynamic export"
    );
    assert_not_no_mangle_item(source_name, source, internal_name);
}
