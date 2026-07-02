use super::*;

#[path = "raw_facade/bootstrap_query.rs"]
mod bootstrap_query;
#[path = "raw_facade/cxx_compat.rs"]
mod cxx_compat;
#[path = "raw_facade/header_forwarding.rs"]
mod header_forwarding;
#[path = "raw_facade/libc_posix.rs"]
mod libc_posix;
#[path = "raw_facade/raw_override.rs"]
mod raw_override;

pub(super) fn assert_raw_facade_boundaries(sources: &UnsafePerformanceSources) {
    bootstrap_query::assert_bootstrap_and_query_boundaries(sources);
    raw_override::assert_raw_override_boundaries(sources);
    libc_posix::assert_libc_and_posix_boundaries(sources);
    cxx_compat::assert_cxx_compatibility_boundaries(sources);
    header_forwarding::assert_header_forwarding_boundaries(sources);
}
