pub(super) fn assert_bitmap_traversal_and_regular_boundaries(
    bitmap_rs: &str,
    arena_rs: &str,
    arena_heap_pages_rs: &str,
    callbacks_rs: &str,
    threadlocal_rs: &str,
) {
    assert!(
        bitmap_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_bitmap_forall_set(")
            && bitmap_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_bitmap_forall_setc_ranges("
            )
            && bitmap_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_bitmap_forall_setc_rangesn("
            )
            && bitmap_rs
                .contains("return mi_bitmap_forall_setc_ranges_typed(bitmap, visit, ArenaPtr::new(arena), arg);")
            && arena_rs.contains("_mi_bitmap_forall_setc_rangesn("),
        "bitmap traversal should keep C's set/range traversal helper boundaries"
    );
    assert!(
        bitmap_rs.contains(
            "pub(crate) unsafe fn mi_bitmap_forall_set_typed(\n    bitmap: *mut mi_bitmap_t,\n    visit: mi_forall_set_fun_t,\n    arena: Option<ArenaPtr>,"
        ) && bitmap_rs.contains("unsafe { mi_bitmap_forall_set_typed(bitmap, visit, ArenaPtr::new(arena), arg) }")
            && bitmap_rs.contains("let index = SliceIndex::new(base_idx + bidx);")
            && bitmap_rs.contains("visit(index.get(), one.get() as usize, arena, arg)")
            && arena_heap_pages_rs.contains("ok = mi_bitmap_forall_set_typed(")
            && arena_heap_pages_rs.contains("Some(arena),"),
        "_mi_bitmap_forall_set should keep its raw extern shim but use typed arena/index wrappers internally"
    );
    assert!(
        bitmap_rs.contains(
            "pub(crate) unsafe fn mi_bitmap_forall_setc_ranges_typed(\n    bitmap: *mut mi_bitmap_t,\n    visit: mi_forall_set_fun_t,\n    arena: Option<ArenaPtr>,"
        ) && bitmap_rs.contains("unsafe { mi_bitmap_forall_setc_ranges_typed(bitmap, visit, ArenaPtr::new(arena), arg) }")
            && bitmap_rs.contains("let range = SliceRange::from_raw(")
            && bitmap_rs.contains("visit(range.start_usize(), range.count_usize(), arena, arg)"),
        "_mi_bitmap_forall_setc_ranges should keep its raw extern shim but use typed arena/range wrappers internally"
    );
    assert!(
        bitmap_rs.contains(
            "pub(crate) unsafe fn mi_bitmap_forall_setc_rangesn_typed(\n    bitmap: *mut mi_bitmap_t,\n    rngslices: SliceCount,"
        ) && bitmap_rs.contains("let rngslices_raw = if rngslices > MI_BFIELD_BITS {")
            && bitmap_rs.contains("crate::internal::mi_assert_internal!(rngslices_raw > 0);")
            && bitmap_rs.contains("let rngslices = SliceCount::new_unchecked(rngslices_raw as u32);")
            && bitmap_rs.contains("mi_bitmap_forall_setc_rangesn_typed(bitmap, rngslices, visit, ArenaPtr::new(arena), arg)")
            && bitmap_rs.contains("let rngslices_count = rngslices;")
            && bitmap_rs.contains("let range = SliceRange::from_raw(idx as u32, rngslices_count.get())")
            && bitmap_rs.contains("visit(range.start_usize(), range.count_usize(), arena, arg)"),
        "_mi_bitmap_forall_setc_rangesn should keep its raw extern shim but use typed range/count wrappers internally"
    );
    for marker in [
        "#[inline(never)]\npub(crate) unsafe fn mi_bitmap_try_find_and_claim(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_size(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_init(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_unsafe_setN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_setN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_clearN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_set(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_clear(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_popcountN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_xsetN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_setN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_clearN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_set(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_clear(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_all_clear(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_clear_once_set(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_bsr(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_popcount(",
        "static MI_BITMAP_TRY_FIND_AND_CLAIM_KEEP",
        "static MI_BITMAP_SIZE_KEEP",
        "static MI_BITMAP_INIT_KEEP",
        "static MI_BITMAP_UNSAFE_SETN_KEEP",
        "static MI_BITMAP_SETN_KEEP",
        "static MI_BITMAP_CLEARN_KEEP",
        "static MI_BITMAP_SET_KEEP",
        "static MI_BITMAP_CLEAR_KEEP",
        "static MI_BITMAP_POPCOUNTN_KEEP",
        "static MI_BITMAP_IS_XSETN_KEEP",
        "static MI_BITMAP_IS_SETN_KEEP",
        "static MI_BITMAP_IS_CLEARN_KEEP",
        "static MI_BITMAP_IS_SET_KEEP",
        "static MI_BITMAP_IS_CLEAR_KEEP",
        "static MI_BITMAP_IS_ALL_CLEAR_KEEP",
        "static MI_BITMAP_CLEAR_ONCE_SET_KEEP",
        "static MI_BITMAP_BSR_KEEP",
        "static MI_BITMAP_POPCOUNT_KEEP",
    ] {
        assert!(
            bitmap_rs.contains(marker),
            "regular bitmap helper boundary marker missing: {marker}"
        );
    }
    assert!(
        bitmap_rs.contains(
            "pub(crate) unsafe fn mi_bitmap_try_find_and_claim(\n    bitmap: *mut mi_bitmap_t,\n    tseq: ThreadSeq,\n    pidx: &mut SliceIndex,"
        )
            && bitmap_rs.contains("claim: ThreadLocalClaimCallback,\n    arena: Option<ArenaPtr>,")
            && bitmap_rs.contains("let mut claim = claim;")
            && bitmap_rs.contains("let mut raw_idx = pidx.get();")
            && bitmap_rs.contains(".then(|| *pidx = SliceIndex::new(raw_idx))")
            && bitmap_rs.contains("(*claim_fun).invoke(slice_index, claim_data.arena,")
            && bitmap_rs.contains("arena: Option<ArenaPtr>,")
            && threadlocal_rs.contains(
                "let claim_fun = ThreadLocalClaimCallback::new(mi_thread_local_claim_fun);"
            )
            && threadlocal_rs.contains(
                "mi_bitmap_try_find_and_claim(slots, ThreadSeq::new(0), &mut idx, claim_fun, None)"
            )
            && threadlocal_rs.contains(
                "mi_key_create(\n            ThreadLocalSlotIndex::new(idx.get()),\n            ThreadLocalKeyVersion::new(version),\n        )"
            )
            && arena_rs.contains(
                "let claim_fun = ThreadLocalClaimCallback::new(mi_arena_try_claim_abandoned);"
            )
            && arena_rs.contains("let mut slice_index = SliceIndex::new(0);")
            && arena_rs.contains("claim_fun,\n                    Some(arena),")
            && arena_rs.contains("let page = mi_arena_page_at_slice(arena, slice_index);")
            && callbacks_rs.contains("impl ThreadLocalClaimCallback")
            && callbacks_rs.contains("arena: Option<ArenaPtr>,")
            && threadlocal_rs.contains("struct ThreadLocalKeepSet(ptr::NonNull<bool>);")
            && threadlocal_rs.contains("unsafe fn from_raw(keep_set: *mut bool) -> Self")
            && threadlocal_rs.contains("fn mi_thread_local_claim_fun_typed(\n    _slice_index: SliceIndex,\n    _arena: Option<ArenaPtr>,\n    keep_set: ThreadLocalKeepSet,")
            && threadlocal_rs.contains("ArenaPtr::new(arena),")
            && threadlocal_rs.contains("ThreadLocalKeepSet::from_raw(keep_set),")
            && !bitmap_rs.contains("*mut mi_claim_fun_t"),
        "thread-local claim callbacks should route through typed callback wrappers below raw typedefs"
    );
}

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
        "#[inline(never)]\nfn mi_bitmap_chunkmap_try_clear(",
        "#[inline(never)]\nfn mi_bbitmap_set_chunk_bin(",
        "#[inline(never)]\nfn mi_bbitmap_chunkmap_try_clear(",
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
