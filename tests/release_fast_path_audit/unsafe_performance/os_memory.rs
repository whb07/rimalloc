use super::*;

#[path = "os_memory/os_wrappers.rs"]
mod os_wrappers;
#[path = "os_memory/platform_helpers.rs"]
mod platform_helpers;

pub(super) fn assert_os_memory_and_platform_boundaries(sources: &UnsafePerformanceSources) {
    platform_helpers::assert_os_memory_and_platform_boundaries(sources);
}
