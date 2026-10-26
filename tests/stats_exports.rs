use rimalloc::raw::{MI_BIN_HUGE, mi_stats_get_bin_size};

#[test]
fn stats_bin_size_export_links_and_handles_bounds() {
    unsafe {
        assert!(mi_stats_get_bin_size(0) > 0);
        assert!(mi_stats_get_bin_size(1) > 0);
        assert!(mi_stats_get_bin_size(1) >= mi_stats_get_bin_size(0));
        assert!(mi_stats_get_bin_size(MI_BIN_HUGE) > 0);
        assert_eq!(mi_stats_get_bin_size(MI_BIN_HUGE + 1), 0);
    }
}
