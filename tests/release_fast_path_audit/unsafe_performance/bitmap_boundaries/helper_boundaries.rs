pub(super) fn assert_bitmap_helper_boundaries(bitmap_rs: &str) {
    for marker in [
        "#[inline(never)]\npub unsafe fn mi_bbitmap_debug_get_bin(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_size(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_init(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_unsafe_setN(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_bsr_inv(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_setN(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_clearNC(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_is_xsetN(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear8(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clearNX(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clearNC(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clearN_(",
        "static MI_BBITMAP_DEBUG_GET_BIN_KEEP",
        "static MI_BBITMAP_SIZE_KEEP",
        "static MI_BBITMAP_INIT_KEEP",
        "static MI_BBITMAP_UNSAFE_SETN_KEEP",
        "static MI_BBITMAP_BSR_INV_KEEP",
        "static MI_BBITMAP_SETN_KEEP",
        "static MI_BBITMAP_TRY_CLEARNC_KEEP",
        "static MI_BBITMAP_IS_XSETN_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEAR_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEAR8_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEARNX_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEARNC_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEARN_KEEP",
    ] {
        assert!(
            bitmap_rs.contains(marker),
            "bbitmap helper boundary marker missing: {marker}"
        );
    }
    let bbitmap_find_start = bitmap_rs
        .find("#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear(")
        .expect("find mi_bbitmap_try_find_and_clear");
    let bbitmap_find_end = bitmap_rs[bbitmap_find_start..]
        .find("#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear8(")
        .map(|offset| bbitmap_find_start + offset)
        .expect("find end of mi_bbitmap_try_find_and_clear");
    let bbitmap_find_body = &bitmap_rs[bbitmap_find_start..bbitmap_find_end];
    assert!(
        !bbitmap_find_body.contains("mi_bbitmap_try_find_and_clear_generic(")
            && bbitmap_find_body.contains("mi_bchunk_try_find_and_clear(chunk, &mut cidx)")
            && bbitmap_find_body
                .contains("[(MI_CBIN_SMALL, cmap_small), (MI_CBIN_NONE, cmap_none)]"),
        "single-slice bbitmap find/clear should keep C's specialized static-inline wrapper shape"
    );
    for marker in [
        "#[inline(never)]\nfn mi_bchunk_xsetNC(",
        "#[inline(never)]\nfn mi_bchunk_popcountNC(",
        "#[inline(never)]\nfn mi_bchunk_is_xsetNC(",
        "#[inline(never)]\nfn mi_bchunk_try_clearNC(",
        "#[inline(never)]\nfn mi_bchunk_try_find_and_clear8(",
        "#[inline(never)]\nfn mi_bchunk_try_find_and_clearNX(",
        "#[inline(never)]\n#[allow(clippy::many_single_char_names)]\nfn mi_bchunk_try_find_and_clearNC(",
        "#[inline(never)]\npub(super) fn mi_bitmap_chunkmap_try_clear(",
        "#[inline(never)]\npub(super) fn mi_bbitmap_set_chunk_bin(",
        "#[inline(never)]\npub(super) fn mi_bbitmap_chunkmap_try_clear(",
        "static MI_BCHUNK_XSETNC_KEEP",
        "static MI_BCHUNK_POPCOUNTNC_KEEP",
        "static MI_BCHUNK_IS_XSETNC_KEEP",
        "static MI_BCHUNK_TRY_CLEARNC_KEEP",
        "static MI_BCHUNK_TRY_FIND_AND_CLEAR8_KEEP",
        "static MI_BCHUNK_TRY_FIND_AND_CLEARNX_KEEP",
        "static MI_BCHUNK_TRY_FIND_AND_CLEARNC_KEEP",
        "static MI_BITMAP_CHUNKMAP_TRY_CLEAR_KEEP",
        "static MI_BBITMAP_SET_CHUNK_BIN_KEEP",
        "static MI_BBITMAP_CHUNKMAP_TRY_CLEAR_KEEP",
    ] {
        assert!(
            bitmap_rs.contains(marker),
            "bchunk bitmap helper boundary marker missing: {marker}"
        );
    }
}
