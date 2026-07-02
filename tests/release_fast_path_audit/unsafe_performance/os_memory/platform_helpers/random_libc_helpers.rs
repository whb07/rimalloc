pub(super) fn assert_random_and_libc_boundaries(random_rs: &str, libc_rs: &str) {
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
}
