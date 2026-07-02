use super::*;

pub(super) fn assert_os_memory_and_platform_boundaries(sources: &UnsafePerformanceSources) {
    let unsafe_allowlist = sources.unsafe_allowlist.as_str();
    let internal = sources.internal.as_str();
    let arena_heap_pages_rs = sources.arena_heap_pages_rs.as_str();
    let libc_rs = sources.libc_rs.as_str();
    let os_rs = sources.os_rs.as_str();
    let options_rs = sources.options_rs.as_str();
    let random_rs = sources.random_rs.as_str();
    let stats_rs = sources.stats_rs.as_str();
    let init_rs = sources.init_rs.as_str();
    let heap_rs = sources.heap_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();
    let prim_unix_rs = sources.prim_unix_rs.as_str();

    assert!(
        init_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_is_main_thread() -> bool")
            && init_rs.contains("#[inline(always)]\nfn mi_is_main_thread_fast() -> bool")
            && init_rs.contains("let main_tid = unsafe { tld_main.thread_id };")
            && init_rs.contains("main_tid == 0 || main_tid == current_thread_id()")
            && init_rs.contains("pub extern \"C\" fn _mi_is_main_thread() -> bool {\n    mi_is_main_thread_fast()\n}")
            && !init_rs.contains("static MAIN_THREAD_ID")
            && !init_rs.contains("mi_mark_main_thread")
            && init_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_preloading() -> bool")
            && init_rs.contains("OS_PRELOADING.load(Ordering::Relaxed)")
            && prim_rs.contains("#[cfg(mi_c_malloc_fast)]")
            && prim_rs.contains("core::arch::global_asm!(")
            && prim_rs.contains("\".hidden _mi_is_redirected\"")
            && prim_rs.contains("\"_mi_is_redirected:\"")
            && prim_rs.contains("\"xor eax,eax\"")
            && prim_rs.contains("pub(crate) fn _mi_is_redirected() -> bool;")
            && prim_rs.contains("#[cfg(not(mi_c_malloc_fast))]")
            && prim_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_is_redirected() -> bool")
            && prim_rs.contains("false")
            && init_rs.contains("pub extern \"C\" fn mi_is_redirected() -> bool {\n    unsafe { crate::prim::_mi_is_redirected() }\n}"),
        "lifecycle predicates should keep C's _mi_is_main_thread/_mi_preloading/_mi_is_redirected helper boundaries"
    );
    assert!(
        prim_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_allocator_init(message: *mut *const c_char) -> bool"
        ) && prim_rs.contains("unsafe { *message = ptr::null() };")
            && prim_rs.contains("\".hidden _mi_allocator_done\"")
            && prim_rs.contains("\"_mi_allocator_done:\"")
            && prim_rs.contains("pub(crate) fn _mi_allocator_done();")
            && prim_rs.contains("#[cfg(not(mi_c_malloc_fast))]")
            && prim_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_allocator_done()")
            && options_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_options_init()")
            && options_rs.contains("for i in 0..MI_OPTION_COUNT")
            && options_rs.contains("mi_max_error_count.store(max_errors as isize, Ordering::Release);")
            && options_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_options_post_init()")
            && options_rs.contains("mi_add_stderr_output();")
            && options_rs.contains("mi_options_print();")
            && init_rs.contains("_mi_options_post_init();")
            && init_rs.contains("if unsafe { crate::prim::_mi_is_redirected() }")
            && init_rs.contains("_mi_verbose_message(format_args!(\"malloc is redirected.\\n\"));")
            && init_rs.contains("let mut msg: *const c_char = ptr::null();")
            && init_rs.contains("unsafe { _mi_allocator_init(ptr::addr_of_mut!(msg)) };")
            && init_rs.contains("mi_option_is_enabled(mi_option_t::mi_option_show_errors)")
            && init_rs.contains("unsafe { _mi_fputs(None, ptr::null_mut(), ptr::null(), msg) };")
            && init_rs.contains("_mi_allocator_done();"),
        "process init/done should keep C's allocator/options helper boundaries and redirector hook sequence"
    );
    assert!(
        options_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_fputs(")
            && options_rs.contains("#[link_name = \"stdout\"]")
            && options_rs.contains("#[link_name = \"stderr\"]")
            && options_rs.contains("fn mi_output_is_c_file(out: mi_output_fun) -> bool")
            && options_rs.contains("p == unsafe { C_STDOUT } || p == unsafe { C_STDERR }")
            && options_rs.contains("let use_default = match out {")
            && options_rs.contains("None => true,")
            && options_rs.contains("Some(out) => mi_output_is_c_file(out),")
            && options_rs.contains(
                "fn mi_out_get_default(parg: Option<&mut OutputCallbackArg>) -> OutputCallback"
            )
            && options_rs.contains("let default_out = mi_out_get_default(Some(&mut default_arg));")
            && options_rs.contains("unsafe { default_out.invoke(message, default_arg) };")
            && options_rs.contains("let out = OutputCallback::new(Some(out));")
            && options_rs.contains("let arg = OutputCallbackArg::from_raw(arg);")
            && options_rs.contains("unsafe { out.invoke(message, arg) };")
            && stats_rs.contains("_mi_fputs(buf.out, buf.arg, ptr::null(), buf.buf);")
            && init_rs.contains("unsafe { _mi_fputs(None, ptr::null_mut(), ptr::null(), msg) };"),
        "_mi_fputs should keep C's local helper boundary and stdout/stderr default-output compatibility"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_getenv(")
            && libc_rs.contains("if name.is_null() || result.is_null() || result_size < 64")
            && libc_rs.contains("crate::prim::_mi_prim_getenv(name, result, result_size)")
            && prim_unix_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_getenv(")
            && prim_unix_rs.contains("for i in 0..10000usize")
            && prim_unix_rs.contains("unsafe { _mi_strnicmp(name, s, len) == 0")
            && prim_unix_rs.contains("if _mi_preloading() {\n        return false;")
            && prim_unix_rs.contains("s = unsafe { getenv(buf.as_ptr()) };")
            && options_rs.contains("crate::libc::_mi_getenv("),
        "_mi_getenv and _mi_prim_getenv should keep C's environment helper boundaries and option parser dependency"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_popcount_generic(")
            && libc_rs.contains("if x <= 1 {\n        return x;")
            && libc_rs.contains("if !x == 0 {\n        return MI_SIZE_BITS;")
            && libc_rs.contains("mi_popcount_generic64(x as u64)")
            && libc_rs.contains(
                "static MI_POPCOUNT_GENERIC_KEEP: extern \"C\" fn(usize) -> usize = _mi_popcount_generic;"
            ),
        "_mi_popcount_generic should keep C's local noinline generic popcount boundary"
    );
    for marker in [
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_mem_init(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_alloc(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_commit(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_reset(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_protect(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_reuse(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_numa_node_count(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_clock_now(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_process_info(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_out_stderr(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_random_buf(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_init_auto_done(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_done_auto_done(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_yield(",
        "static MI_PRIM_MEM_INIT_KEEP",
        "static MI_PRIM_ALLOC_KEEP",
        "static MI_PRIM_COMMIT_KEEP",
        "static MI_PRIM_RESET_KEEP",
        "static MI_PRIM_PROTECT_KEEP",
        "static MI_PRIM_REUSE_KEEP",
        "static MI_PRIM_ALLOC_HUGE_OS_PAGES_KEEP",
        "static MI_PRIM_NUMA_NODE_COUNT_KEEP",
        "static MI_PRIM_CLOCK_NOW_KEEP",
        "static MI_PRIM_PROCESS_INFO_KEEP",
        "static MI_PRIM_OUT_STDERR_KEEP",
        "static MI_PRIM_RANDOM_BUF_KEEP",
        "static MI_PRIM_THREAD_INIT_AUTO_DONE_KEEP",
        "static MI_PRIM_THREAD_DONE_AUTO_DONE_KEEP",
        "static MI_PRIM_THREAD_ASSOCIATE_DEFAULT_THEAP_KEEP",
        "static MI_PRIM_THREAD_IS_IN_THREADPOOL_KEEP",
        "static MI_PRIM_THREAD_YIELD_KEEP",
    ] {
        assert!(
            prim_unix_rs.contains(marker),
            "Unix primitive helper boundary marker missing: {marker}"
        );
    }
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/unix.rs\t"))
            && prim_unix_rs
                .matches("SAFETY[AUDIT:unix-primitive-backend]")
                .count()
                >= 85,
        "Unix primitive backend unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    for marker in [
        "#[inline(never)]\npub(crate) extern \"C\" fn _mi_os_random_weak(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_split(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_next(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_init(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_init_weak(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_reinit_if_weak(",
    ] {
        assert!(
            random_rs.contains(marker),
            "random helper boundary marker missing: {marker}"
        );
    }
    assert!(
        random_rs.contains("SAFETY[AUDIT:random-context-boundaries]"),
        "random context helper unsafe operations should retain operation-level audit markers"
    );
    heap_page_visitors::assert_heap_page_visitor_boundaries(
        heap_page_visitors::HeapPageVisitorSources {
            arena_heap_pages_rs,
            heap_rs,
        },
    );
    assert!(
        !random_rs.contains("static MI_OS_RANDOM_WEAK_KEEP")
            && !random_rs.contains("static MI_RANDOM_SPLIT_KEEP")
            && !random_rs.contains("static MI_RANDOM_NEXT_KEEP")
            && !random_rs.contains("static MI_RANDOM_INIT_KEEP")
            && !random_rs.contains("static MI_RANDOM_INIT_WEAK_KEEP")
            && !random_rs.contains("static MI_RANDOM_REINIT_IF_WEAK_KEEP"),
        "random helper retention function pointers should stay absent; they force indirect calls in the release cdylib"
    );

    for marker in [
        "#[inline(never)]\npub(crate) extern \"C\" fn _mi_toupper(",
        "#[inline(never)]\n#[allow(clippy::many_single_char_names)]\npub(crate) unsafe extern \"C\" fn _mi_strnicmp(",
        "#[inline(never)]\n#[allow(clippy::many_single_char_names)]\npub(crate) unsafe extern \"C\" fn _mi_streq(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlcpy(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlcat(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strnlen(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlen(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strnstr(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_vsnprintf(",
        "#[inline(never)]\npub(crate) fn _mi_snprintf(",
        "static MI_TOUPPER_KEEP",
        "static MI_STRNICMP_KEEP",
        "static MI_STREQ_KEEP",
        "static MI_STRLCPY_KEEP",
        "static MI_STRLCAT_KEEP",
        "static MI_STRNLEN_KEEP",
        "static MI_STRLEN_KEEP",
        "static MI_STRNSTR_KEEP",
        "static MI_VSNPRINTF_KEEP",
        "static MI_SNPRINTF_KEEP",
    ] {
        assert!(
            libc_rs.contains(marker),
            "libc helper boundary marker missing: {marker}"
        );
    }

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
        ("os-memory-config-boundary", 20),
        ("os-memory-guard-boundary", 6),
        ("os-memory-commit-boundary", 12),
        ("os-memory-free-boundary", 8),
        ("os-memory-allocation-boundary", 50),
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
            && !os_rs.contains("SAFETY[AUDIT:os-memory-boundary]"),
        "OS memory unsafe boundaries should stay split, covered by local operation markers, and free of migration allowlist entries"
    );
}
