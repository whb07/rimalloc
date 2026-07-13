pub(super) fn assert_page_reclaim_atomic_paths(
    page_rs: &str,
    atomic_types: &str,
    internal: &str,
    internal_page_flags: &str,
    page_flags_rs: &str,
    arena_rs: &str,
    free_rs: &str,
) {
    let internal_page_boundary = format!("{internal}\n{internal_page_flags}");
    assert!(
    page_rs.contains(
        "#[inline(never)]\npub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
    )
        && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_page_reclaim_raw")
        && page_rs.contains("static MI_THEAP_PAGE_RECLAIM_KEEP")
        && page_rs.contains("mi_theap_page_reclaim_raw;")
        && page_rs.contains("let _ = _mi_theap_page_reclaim(wrapper, theap_wrapper);")
        && page_rs
            .contains("let wrapper = crate::core_types::assume_owned_abandoned_page(page_ptr);")
        && !page_rs.contains("pub unsafe extern \"C\" fn _mi_theap_page_reclaim")
        && atomic_types.contains("pub(crate) unsafe fn load(self) -> PageThreadId")
        && atomic_types.contains("pub(crate) unsafe fn store_owner(self, owner: PageOwnerId)")
        && atomic_types.contains("pub(crate) unsafe fn compare_exchange_owner(\n        self,\n        expected: &mut PageThreadId,")
        && internal_page_boundary.contains("let atomic_tid = page.thread_id_cell();")
        && internal_page_boundary.contains("atomic_tid.load().raw()")
        && internal_page_boundary.contains("let mut old = atomic_tid.load();")
        && internal_page_boundary.contains("atomic_tid.compare_exchange_owner(&mut old, new)")
        && !internal_page_boundary.contains("AtomicPageThreadId::from_field(")
        && internal.contains("let new = old.with_owner(crate::core_types::PageOwnerId::AbandonedMapped);")
        && internal.contains("let new = old.with_owner(crate::core_types::PageOwnerId::Abandoned);")
        && page_flags_rs
            .contains("let Some(flag) = crate::core_types::PageFlags::new(newflag) else")
        && page_flags_rs.contains("let old = atomic_tid.set_flags(flag, set);")
        && page_flags_rs.contains("let tid = atomic_tid.load();")
        && page_flags_rs.contains("let mut xtid_old = atomic_tid.load();")
        && page_flags_rs.contains("let xtid = xtid_old.with_owner(owner);")
        && page_flags_rs.contains("atomic_tid.compare_exchange_owner(&mut xtid_old, xtid)")
        && arena_rs.contains("unsafe { atomic_tid.store_owner(crate::core_types::PageOwnerId::AbandonedMapped) };")
        && !internal_page_boundary.contains("atomic_tid.load_relaxed().raw()")
        && !internal_page_boundary.contains("atomic_tid.compare_exchange_release(&mut old, new)")
        && !page_flags_rs.contains("atomic_tid.compare_exchange_release(&mut xtid_old, xtid)")
        && !arena_rs.contains("atomic_tid.set_owner_release(crate::core_types::PageOwnerId::AbandonedMapped)")
        && !internal_page_boundary.contains("mi_atomic_or_relaxed(&mut (*page).xthread_id")
        && !internal_page_boundary.contains("mi_atomic_and_relaxed(&mut (*page).xthread_id")
        && !page_flags_rs.contains("mi_atomic_cas_weak_release(&mut (*page).xthread_id")
        && !page_flags_rs.contains("mi_atomic_or_relaxed(&mut (*page).xthread_id")
        && !page_flags_rs.contains("mi_atomic_and_relaxed(&mut (*page).xthread_id")
        && !arena_rs.contains("MI_THREADID_ABANDONED_MAPPED | flags")
        && page_rs.contains("let atomic_tf = page_ptr.thread_free_cell();")
        && page_rs.contains("let remote = atomic_tf.detach_head();")
        && page_rs.contains("collect_remote_for_owned_abandoned_page(&page_wrapper, remote, local)")
        && !page_rs.contains("atomic_tf.take_head_preserving_owner()")
        && !page_rs.contains("AtomicThreadFree::from_field(")
        && !page_rs.contains("addr_of_mut!((*page).xthread_free)")
        && atomic_types.contains("pub(crate) fn try_push(")
        && atomic_types.contains(
            "pub(crate) unsafe fn detach_head(self) -> DetachedRemoteList"
        )
        && atomic_types.contains("pub(crate) unsafe fn claim_acq_rel(self) -> bool")
        && atomic_types.contains(
            "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool"
        )
        && atomic_types.contains("fn field(self) -> AtomicUsizeField")
        && atomic_types.contains("self.field().compare_exchange_weak(")
        && atomic_types.contains("self.field().fetch_or(1, Ordering::AcqRel)")
        && atomic_types.contains("AtomicUsize::from_ptr(self.0)")
        && !atomic_types.contains("self.0.cast::<core::sync::atomic::AtomicUsize>()")
        && !atomic_types.contains("as_atomic(")
        && !atomic_types.contains("atomic_ref(")
        && !atomic_types.contains("-> &'static Atomic")
        && atomic_types.contains("Atomic access to a raw `usize` field.")
        && atomic_types.contains("Atomic access to a raw `i64` field.")
        && atomic_types.contains("Atomic access to a raw pointer field.")
        && internal.contains("crate::core_types::ThreadFreeWord::from_raw(tf).head_raw()")
        && internal.contains("crate::core_types::ThreadFreeWord::from_raw(tf).is_owned()")
        && internal.contains("crate::core_types::ThreadFreeWord::from_parts(")
        && internal_page_boundary.contains("pub unsafe fn mi_page_thread_free(page: *const mi_page_t) -> *mut mi_block_t")
        && internal_page_boundary.contains("atomic_tf.load_relaxed().head_raw()")
        && internal_page_boundary.contains("pub unsafe fn mi_page_is_owned(page: *const mi_page_t) -> bool")
        && internal_page_boundary.contains("atomic_tf.load_relaxed().is_owned()")
        && internal_page_boundary.contains("let atomic_tf = page_ptr.thread_free_cell();")
        && internal_page_boundary.contains("let atomic_tf = page.thread_free_cell();")
        && internal_page_boundary.contains("atomic_tf.claim_acq_rel()")
        && !internal_page_boundary.contains("AtomicThreadFree::from_field(")
        && !internal_page_boundary.contains("addr_of_mut!((*page).xthread_free)")
        && !internal_page_boundary.contains("atomic_tf.claim_ownership()")
        && arena_rs.contains("(*page).xthread_free = crate::core_types::ThreadFreeWord::from_parts(\n            None,\n            crate::core_types::ThreadFreeOwner::Unowned,")
        && !arena_rs.contains("(*page).xthread_free = 0")
        && free_rs.contains("let atomic_tf = page_ptr.thread_free_cell();")
        && free_rs.contains("let atomic_tf = page.page().thread_free_cell();")
        && free_rs.contains("match atomic_tf.try_push(expected, bp)")
        && free_rs.contains("while !atomic_tf.release_acq_rel(&mut tf_expect)")
        && free_rs.contains("tf_expect = atomic_tf.load_relaxed();")
        && !free_rs.contains("atomic_tf.compare_exchange_push(")
        && !free_rs.contains("atomic_tf.compare_exchange_release_ownership(")
        && !free_rs.contains("mi_tf_create(expected_thread_free")
        && !free_rs.contains("AtomicThreadFree::from_field(")
        && arena_rs.contains("let atomic_tf = unsafe { page_ptr.thread_free_cell() };")
        && arena_rs.contains("let mut tf_old = atomic_tf.load_relaxed();")
        && arena_rs.contains("while tf_old.head().is_some()")
        && arena_rs.contains("if unsafe { atomic_tf.release_acq_rel(&mut tf_old) }")
        && !arena_rs.contains("atomic_tf.compare_exchange_release_ownership(")
        && !arena_rs.contains("mi_tf_create(ptr::null_mut(), false)")
        && !arena_rs.contains("AtomicThreadFree::from_field(")
        && !arena_rs.contains("addr_of_mut!((*page).xthread_free)")
        && page_rs.contains(
            "unsafe fn mi_page_queue_push_at_end_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
        )
        && page_rs.contains(
            "mi_page_queue_push_at_end_typed(\n            crate::core_types::ThreadHeapLocal::new_unchecked"
        )
        && page_rs.contains("crate::core_types::NormalPageQueue")
        && page_rs.contains("crate::core_types::ActivePage"),
    "_mi_theap_page_reclaim should keep C's local abandoned-page reclaim helper boundary while collecting remote frees through typed wrappers"
);
}
