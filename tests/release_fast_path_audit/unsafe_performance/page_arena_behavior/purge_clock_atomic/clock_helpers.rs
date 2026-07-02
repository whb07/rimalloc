use super::PageArenaBehaviorSources;

pub(super) fn assert_clock_helper_boundaries(sources: &PageArenaBehaviorSources<'_>) {
    assert!(
        sources
            .stats_rs
            .contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_now()")
            && sources
                .stats_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_start()")
            && sources
                .stats_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_end(")
            && sources.stats_rs.contains("let t0 = _mi_clock_now();")
            && sources
                .stats_rs
                .contains("let diff = _mi_clock_now() - t0;")
            && sources
                .stats_rs
                .contains("_mi_clock_now() - start - CLOCK_DIFF.load")
            && sources.os_rs.contains("let start_t = _mi_clock_start();")
            && sources
                .os_rs
                .contains("let mut elapsed = _mi_clock_end(start_t);")
            && sources
                .arena_rs
                .contains("let expire = crate::stats::_mi_clock_now() + delay as i64;")
            && sources
                .arena_rs
                .contains("let now = crate::stats::_mi_clock_now();"),
        "clock helpers should keep C's timing helper boundaries and main callers"
    );
}
