use super::*;

pub(super) fn assert_os_random_no_exports(sources: &UnsafePerformanceSources) {
    let os_rs = sources.os_rs.as_str();
    let prim_rs = sources.prim_rs.as_str();
    let prim_unix_rs = sources.prim_unix_rs.as_str();
    let random_rs = sources.random_rs.as_str();

    for (source_name, source, internal_name) in [
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_free",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_mem_init",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_commit",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reuse",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_decommit",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reset",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_protect",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_numa_node",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_numa_node_count",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_clock_now",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_process_info",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_out_stderr",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_random_buf",
        ),
        (
            "src/platform/unix.rs",
            prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_getenv",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_alloc",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_zalloc",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free_ex",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_page_size",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_good_alloc_size",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_overcommit",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_virtual_reserve",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_virtual_address_bits",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_reuse",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit_ex",
        ),
        (
            "src/platform/os.rs",
            os_rs,
            "pub unsafe extern \"C\" fn _mi_os_purge_ex",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) extern \"C\" fn _mi_os_random_weak",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_split",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_next",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init_weak",
        ),
        (
            "src/internal/random.rs",
            random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_reinit_if_weak",
        ),
        ("src/platform/mod.rs", prim_rs, "fn _mi_allocator_init"),
        ("src/platform/mod.rs", prim_rs, "fn _mi_allocator_done"),
    ] {
        assert_internal_no_export(source_name, source, internal_name);
    }
}
