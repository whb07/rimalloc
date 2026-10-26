// Raw/unsafe stats API example. Prefer `rimalloc::StatsSnapshot`
// unless you need the C-shaped compatibility surface.
fn main() {
    unsafe {
        rimalloc::raw::mi_stats_reset();

        let ptr = rimalloc::raw::mi_malloc(32);
        assert!(!ptr.is_null());
        rimalloc::raw::mi_free(ptr);

        let mut stats = rimalloc::raw::mi_stats_t {
            size: core::mem::size_of::<rimalloc::raw::mi_stats_t>(),
            version: rimalloc::raw::MI_STAT_VERSION,
            ..Default::default()
        };
        assert!(rimalloc::raw::mi_stats_get(&mut stats));
        assert_eq!(stats.version, rimalloc::raw::MI_STAT_VERSION);
    }
}
