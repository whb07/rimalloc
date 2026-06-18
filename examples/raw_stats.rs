// Raw/unsafe stats API example. There is no safe `StatsSnapshot` facade yet.
fn main() {
    unsafe {
        mechanical_mimalloc::raw::mi_stats_reset();

        let ptr = mechanical_mimalloc::raw::mi_malloc(32);
        assert!(!ptr.is_null());
        mechanical_mimalloc::raw::mi_free(ptr);

        let mut stats = mechanical_mimalloc::raw::mi_stats_t {
            size: core::mem::size_of::<mechanical_mimalloc::raw::mi_stats_t>(),
            version: mechanical_mimalloc::raw::MI_STAT_VERSION,
            ..Default::default()
        };
        assert!(mechanical_mimalloc::raw::mi_stats_get(&mut stats));
        assert_eq!(stats.version, mechanical_mimalloc::raw::MI_STAT_VERSION);
    }
}
