pub(super) fn assert_dynamic_threadlocal_boundaries(threadlocal_rs: &str) {
    let split_marker_minimums = [
        ("threadlocal-empty-sentinel-boundary", 1),
        ("threadlocal-tls-shim-boundary", 9),
        ("threadlocal-slot-block-boundary", 7),
        ("threadlocal-claim-callback-boundary", 6),
        ("threadlocal-key-claim-boundary", 1),
        ("threadlocal-free-bitmap-boundary", 5),
        ("threadlocal-global-manager-boundary", 6),
        ("threadlocal-key-access-boundary", 4),
        ("threadlocal-thread-done-boundary", 3),
        ("threadlocal-key-create-boundary", 3),
        ("threadlocal-key-free-boundary", 5),
    ];

    assert!(
        threadlocal_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_locals_init(")
            && threadlocal_rs.contains("static mut mi_thread_locals_lock: mi_lock_t")
            && threadlocal_rs.contains("static mi_thread_locals_initialized: AtomicBool")
            && threadlocal_rs
                .contains(".compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_locals_done(")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_local_set(")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_local_get(")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_locals_thread_done(")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_local_free(")
            && threadlocal_rs.contains("static MI_THREAD_LOCALS_INIT_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCALS_DONE_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCAL_SET_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCAL_GET_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCALS_THREAD_DONE_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCAL_FREE_KEEP")
            && threadlocal_rs.contains("pub extern \"C\" fn _mi_thread_local_create()")
            && threadlocal_rs.contains("_mi_thread_locals_init();"),
        "dynamic thread-local helpers should keep C's local helper boundaries"
    );
    assert!(
        !threadlocal_rs.contains("SAFETY[AUDIT:threadlocal-key-manager]"),
        "dynamic thread-local markers should stay split instead of reusing the old broad key manager ID"
    );
    for (audit_id, minimum) in split_marker_minimums {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = threadlocal_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "dynamic thread-local unsafe boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
    assert!(
        split_marker_minimums
            .into_iter()
            .map(|(audit_id, _)| {
                let marker = format!("SAFETY[AUDIT:{audit_id}]");
                threadlocal_rs.matches(&marker).count()
            })
            .sum::<usize>()
            >= 50,
        "dynamic thread-local unsafe boundaries should stay covered by local operation markers"
    );
    assert!(
        threadlocal_rs.contains("fn mi_thread_locals_expand(")
            && threadlocal_rs.contains("let tls_old = thread_locals_ptr();")
            && threadlocal_rs.contains("mi_rezalloc(old_ptr, size_new)")
            && threadlocal_rs.contains("(*tls).count = count;")
            && threadlocal_rs.contains("set_thread_locals_ptr(tls);"),
        "thread-local slot expansion should keep the current-thread block ownership and publish path"
    );
    assert!(
        threadlocal_rs.contains("fn mi_thread_local_claim() -> mi_thread_local_t")
            && threadlocal_rs.contains(
                "mi_bitmap_try_find_and_claim(slots, ThreadSeq::new(0), &mut idx, claim_fun, None)"
            )
            && threadlocal_rs
                .contains("mi_thread_locals_version.store(version, Ordering::Relaxed);")
            && threadlocal_rs
                .contains("mi_key_create(\n            ThreadLocalSlotIndex::new(idx.get()),")
            && threadlocal_rs.contains("ThreadLocalKeepSet::from_raw(keep_set)"),
        "thread-local key claim should keep bitmap callback wrapping and versioned key creation"
    );
    assert!(
        threadlocal_rs.contains("fn mi_thread_local_create_expand() -> bool")
            && threadlocal_rs.contains("mi_zalloc_aligned(newsize, MI_BCHUNK_SIZE)")
            && threadlocal_rs.contains(
                "ptr::copy_nonoverlapping(slots.cast::<u8>(), newslots.cast::<u8>(), oldsize)"
            )
            && threadlocal_rs.contains("mi_bitmap_init(newslots, newcount, true);")
            && threadlocal_rs
                .contains("mi_bitmap_unsafe_setN(newslots, oldcount, newcount - oldcount);")
            && threadlocal_rs.contains("mi_thread_locals_free.store(newslots, Ordering::Release);"),
        "thread-local bitmap expansion should keep aligned allocation, copy/free handoff, and release publication"
    );
    assert!(
        threadlocal_rs.contains("pub extern \"C\" fn _mi_thread_local_free(")
            && threadlocal_rs
                .contains("mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));")
            && threadlocal_rs.contains(
                "if !slots.is_null() && idx.get() < unsafe { mi_bitmap_max_bits(slots) }"
            )
            && threadlocal_rs.contains("mi_bitmap_set(slots, idx.get());")
            && threadlocal_rs
                .contains("mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));"),
        "thread-local key free should remain lock-protected and return slots through the free bitmap"
    );
    let thread_locals_thread_done_start = threadlocal_rs
        .find("#[inline(never)]\npub extern \"C\" fn _mi_thread_locals_thread_done")
        .expect("find _mi_thread_locals_thread_done");
    let thread_locals_thread_done_end = threadlocal_rs[thread_locals_thread_done_start..]
        .find("#[used]\nstatic MI_THREAD_LOCALS_THREAD_DONE_KEEP")
        .map(|offset| thread_locals_thread_done_start + offset)
        .expect("find _mi_thread_locals_thread_done keep marker");
    let thread_locals_thread_done_body =
        &threadlocal_rs[thread_locals_thread_done_start..thread_locals_thread_done_end];
    assert!(
        thread_locals_thread_done_body.contains("let tls = thread_locals_ptr();")
            && thread_locals_thread_done_body.contains("!tls.is_null() && (*tls).count > 0")
            && thread_locals_thread_done_body.contains("mi_free(tls.cast());")
            && thread_locals_thread_done_body
                .contains("set_thread_locals_ptr(thread_locals_empty_ptr());")
            && !thread_locals_thread_done_body.contains("free_thread_locals("),
        "_mi_thread_locals_thread_done should match C's direct mi_free after the count guard"
    );
}
