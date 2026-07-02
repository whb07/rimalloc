use super::*;

pub(super) fn assert_os_wrapper_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let internal = sources.internal.as_str();
    let libc_rs = sources.libc_rs.as_str();
    let os_rs = sources.os_rs.as_str();
    let options_rs = sources.options_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let prim_unix_rs = sources.prim_unix_rs.as_str();

    assert!(
        os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_numa_node_count() -> i32")
            && os_rs.contains(
                "static MI_OS_NUMA_NODE_COUNT_KEEP: unsafe extern \"C\" fn() -> i32 = _mi_os_numa_node_count;"
            )
            && prim_unix_rs.contains("#[inline(never)]\nunsafe fn unix_mmap_prim_aligned(")
            && prim_unix_rs.contains("static UNIX_MMAP_PRIM_ALIGNED_KEEP: unsafe fn("),
        "NUMA count and aligned mmap helpers should keep C's local OS helper boundaries"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\nunsafe fn mi_out_num(")
            && libc_rs.contains("static MI_OUT_NUM_KEEP: unsafe fn(")
            && stats_rs.contains("#[inline(never)]\nfn mi_json_buf_print_value(")
            && stats_rs.contains(
                "static MI_JSON_BUF_PRINT_VALUE_KEEP: fn(&mut mi_json_buf_t, &str, i64) = mi_json_buf_print_value;"
            ),
        "number formatting and JSON scalar output should keep C's local formatting helper boundaries"
    );
    assert!(
        options_rs
            .contains("#[inline(never)]\npub(crate) fn _mi_trace_message(args: fmt::Arguments<'_>)")
            && options_rs.contains("mi_option_get(mi_option_t::mi_option_verbose) <= 1")
            && options_rs.contains(
                "static MI_TRACE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_trace_message;"
            )
            && options_rs.contains(
                "#[inline(never)]\npub(crate) fn _mi_verbose_message(args: fmt::Arguments<'_>)"
            )
            && options_rs.contains("!mi_option_is_enabled(mi_option_t::mi_option_verbose)")
            && options_rs.contains(
                "static MI_VERBOSE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_verbose_message;"
            )
            && !internal.contains("macro_rules! mi_trace_message"),
        "_mi_trace_message and _mi_verbose_message should keep C's local message helper boundaries without stale extern declarations or macro exports"
    );
    assert!(
        options_rs.contains(
            "#[inline(never)]\npub(crate) fn _mi_warning_message_fmt(args: fmt::Arguments<'_>)"
        ) && options_rs.contains(
            "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_warning_message(fmt: *const c_char)"
        ) && options_rs.contains(
            "static MI_WARNING_MESSAGE_FMT_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_warning_message_fmt;"
        ) && options_rs.contains(
            "static MI_WARNING_MESSAGE_KEEP: unsafe extern \"C\" fn(*const c_char) = _mi_warning_message;"
        ) && options_rs.contains(
            "#[inline(never)]\npub(crate) fn _mi_error_message_fmt(err: Errno, args: fmt::Arguments<'_>)"
        ) && options_rs.contains(
            "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_error_message(err: c_int, fmt: *const c_char)"
        ) && options_rs.contains(
            "static MI_ERROR_MESSAGE_FMT_KEEP: for<'a> fn(Errno, fmt::Arguments<'a>) = _mi_error_message_fmt;"
        ) && options_rs.contains(
            "static MI_ERROR_MESSAGE_KEEP: unsafe extern \"C\" fn(c_int, *const c_char) = _mi_error_message;"
        ) && !internal.contains("pub fn _mi_warning_message(")
            && !internal.contains("pub fn _mi_error_message("),
        "_mi_warning_message and _mi_error_message should keep C's local message helper boundaries without stale extern declarations"
    );
    assert!(
        os_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_has_overcommit()"
        ) && os_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_has_virtual_reserve()"
        ) && os_rs
            .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_large_page_size()")
            && os_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_minimal_purge_size()"
            )
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_guard_page_size()")
            && os_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_virtual_address_bits()"
            )
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_canuse_large_page(")
            && os_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_good_alloc_size("
            )
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_init()")
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_get_aligned_hint(")
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_size()"
            )
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_set_at("
            )
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_reset_at("
            )
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_commit(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_decommit(")
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_reuse(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_purge(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_protect(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_unprotect(")
            && os_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_free_ex(")
            && os_rs.contains("unsafe fn mi_os_current_subproc() -> Option<BorrowedSubproc>")
            && os_rs.contains(
                "unsafe fn mi_os_subproc_stats_mut(subproc: BorrowedSubproc) -> *mut mi_stats_t"
            )
            && os_rs.contains(
                "unsafe fn mi_os_prim_free(\n    memid: OsMemId,\n    commit_size: CommittedBytes,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains("let subproc = subproc.or_else(|| mi_os_current_subproc());")
            && os_rs.contains("let MemIdView::Os(memid) = MemId::view(memid) else")
            && os_rs.contains(
                "let subproc = SubprocPtr::new(subproc).map(|subproc| unsafe { BorrowedSubproc::new(subproc) });"
            )
            && os_rs.contains("let addr = OsMemPtr::new(addr);")
            && os_rs.contains("let size = ReservedBytes::new(size);")
            && os_rs.contains(
                "unsafe { _mi_os_free_ex_typed(addr, size, still_committed, memid, subproc) };"
            )
            && os_rs.contains(
                "unsafe fn _mi_os_free_ex_typed(\n    addr: Option<OsMemPtr>,\n    size: ReservedBytes,\n    still_committed: bool,\n    os: OsMemId,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains("if matches!(&os.kind, OsMemKind::Huge) {")
            && os_rs.contains(
                "unsafe {\n            mi_os_free_huge_os_pages(OsMemPtr::new(base), ReservedBytes::new(csize), subproc)\n        };"
            )
            && os_rs.contains("} else {\n        let committed = if still_committed {")
            && os_rs.contains("unsafe { mi_os_prim_free(memid, committed, subproc) };")
            && os_rs.contains(
                "unsafe fn mi_os_free_huge_os_pages(\n    p: Option<OsMemPtr>,\n    size: ReservedBytes,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains(
                "mi_os_prim_free(memid, CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE), subproc)"
            )
            && os_rs.contains(
                "mi_os_prim_free(memid, CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE), None);"
            )
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_zalloc(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_alloc_aligned(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_numa_node()")
            && os_rs.contains("static MI_OS_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_GUARD_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_ALLOC_KEEP")
            && os_rs.contains("static MI_OS_FREE_KEEP")
            && os_rs.contains("static MI_OS_REUSE_KEEP")
            && os_rs.contains("static MI_OS_PURGE_KEEP")
            && os_rs.contains("static MI_OS_PROTECT_KEEP")
            && os_rs.contains("static MI_OS_UNPROTECT_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_SET_AT_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_RESET_BEFORE_KEEP"),
        "OS helper wrappers should keep C's local helper boundaries and retained wrapper set"
    );
    for (audit_id, min_count) in [
        ("os-memory-config-state-boundary", 5),
        ("os-memory-subproc-stats-boundary", 2),
        ("os-memory-page-size-boundary", 10),
        ("os-memory-aligned-hint-boundary", 3),
        ("os-memory-page-align-boundary", 4),
        ("os-memory-guard-boundary", 6),
        ("os-memory-commit-boundary", 9),
        ("os-memory-purge-protect-boundary", 10),
        ("os-memory-free-boundary", 8),
        ("os-memory-primitive-allocation-boundary", 4),
        ("os-memory-aligned-allocation-boundary", 18),
        ("os-memory-memid-provenance-boundary", 6),
        ("os-memory-zeroing-boundary", 5),
        ("os-memory-request-allocation-boundary", 2),
        ("os-memory-public-export-boundary", 12),
        ("os-memory-retained-wrapper-boundary", 9),
        ("os-memory-huge-page-boundary", 10),
        ("os-memory-numa-boundary", 6),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            os_rs.matches(&marker).count() >= min_count,
            "OS memory unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/os.rs\t"))
            && !os_rs.contains("SAFETY[AUDIT:os-memory-boundary]")
            && !os_rs.contains("SAFETY[AUDIT:os-memory-allocation-boundary]"),
        "OS memory unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
}
