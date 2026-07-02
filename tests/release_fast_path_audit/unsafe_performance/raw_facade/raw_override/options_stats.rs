use super::*;

pub(super) fn assert_options_stats_no_exports(sources: &UnsafePerformanceSources) {
    let options_rs = sources.options_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();

    for (source_name, source, internal_name) in [
        (
            "src/allocator/options.rs",
            options_rs,
            "fn _mi_warning_message",
        ),
        (
            "src/allocator/options.rs",
            options_rs,
            "fn _mi_error_message",
        ),
        (
            "src/allocator/options.rs",
            options_rs,
            "fn _mi_option_get_fast",
        ),
        ("src/allocator/options.rs", options_rs, "fn _mi_assert_fail"),
        (
            "src/allocator/options.rs",
            options_rs,
            "pub(crate) fn _mi_raw_message",
        ),
        (
            "src/allocator/options.rs",
            options_rs,
            "pub(crate) fn _mi_message",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub unsafe extern \"C\" fn _mi_stats_merge_into",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub extern \"C\" fn _mi_stats_init",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase",
        ),
        (
            "src/allocator/stats.rs",
            stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease",
        ),
    ] {
        assert_internal_no_export(source_name, source, internal_name);
    }
}
