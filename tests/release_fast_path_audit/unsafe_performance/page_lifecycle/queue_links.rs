pub(super) fn assert_page_queue_impl_uses_typed_links() {
    let page_rs = include_str!("../../../../src/allocator/page.rs");
    let page_queue_impl_rs = include_str!("../../../../src/allocator/page/queues.rs");
    assert!(
        page_queue_impl_rs.contains(".queue_next()")
            && page_queue_impl_rs.contains(".set_queue_next(")
            && page_queue_impl_rs.contains(".queue_prev()")
            && page_queue_impl_rs.contains(".set_queue_prev(")
            && !page_queue_impl_rs.contains("(*page).next")
            && !page_queue_impl_rs.contains("(*page).prev")
            && !page_queue_impl_rs.contains("(*page_raw).next")
            && !page_queue_impl_rs.contains("(*page_raw).prev")
            && !page_queue_impl_rs.contains("(*first.as_ptr()).next")
            && !page_queue_impl_rs.contains("(*first.as_ptr()).prev")
            && !page_queue_impl_rs.contains("(*last.as_ptr()).next")
            && !page_queue_impl_rs.contains("(*last.as_ptr()).prev")
            && !page_queue_impl_rs.contains("(*append_first.as_ptr()).next")
            && !page_queue_impl_rs.contains("(*append_first.as_ptr()).prev"),
        "page queue link mutations should use typed PagePtr queue-link helpers"
    );
    assert!(
        page_rs.contains("mi_theap_queue_index(")
            && page_rs.contains("mi_theap_direct_cache_start(")
            && page_queue_impl_rs.contains("pub(super) unsafe fn mi_theap_queue_index(")
            && page_queue_impl_rs.contains("pub(super) unsafe fn mi_theap_direct_cache_start(")
            && !page_rs.contains("offset_from(ptr::addr_of!((*theap).pages[0]))")
            && !page_rs.contains("ptr::addr_of_mut!((*theap_raw).pages[0])"),
        "page queue table geometry should stay isolated in page/queues.rs"
    );
}
