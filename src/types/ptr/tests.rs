use super::*;
use core::mem::MaybeUninit;

#[test]
fn page_ptr_field_helpers_read_and_write_hot_fields() {
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    let raw_page = raw_page.as_mut_ptr();
    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: `raw_page` points at
    // stack-local page storage that remains live for this test.
    let page = unsafe { PagePtr::new_unchecked(raw_page) };

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: the page wrapper and raw page
    // field writes target only the stack-local page fixture above.
    unsafe {
        page.set_block_size(BlockSize::new_unchecked(64));
        page.set_used(UsedBlockCount::new(3));
        ptr::write(ptr::addr_of_mut!((*raw_page).reserved), 7);
        ptr::write(ptr::addr_of_mut!((*raw_page).capacity), 5);
    }

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: the page wrapper still points
    // at the initialized stack-local fixture fields written above.
    unsafe {
        assert_eq!(page.block_size().get(), 64);
        assert_eq!(page.used().get(), 3);
        assert_eq!(page.capacity().get(), 5);
        assert_eq!(page.reserved().get(), 7);
    }

    let mut free_block = MaybeUninit::<mi_block_t>::zeroed();
    let mut local_block = MaybeUninit::<mi_block_t>::zeroed();
    let free_head = free_block.as_mut_ptr();
    let local_head = local_block.as_mut_ptr();

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: the free-list heads point at
    // stack-local block fixtures that remain live until after the assertions.
    unsafe {
        page.set_free(AvailableFreeList::from_raw(free_head));
        page.set_local_free(LocalFreeList::from_raw(local_head));
    }

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: the page fixture stores only
    // the stack-local block pointers assigned above.
    unsafe {
        assert_eq!(page.free().0.map(BlockPtr::as_ptr), Some(free_head));
        assert_eq!(page.local_free().0.map(BlockPtr::as_ptr), Some(local_head));
    }

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: clearing the free-list fields
    // mutates only the stack-local page fixture.
    unsafe {
        page.set_free(AvailableFreeList::from_raw(ptr::null_mut()));
        page.set_local_free(LocalFreeList::from_raw(ptr::null_mut()));
    }

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: the page fixture is still live
    // and the free-list fields were just cleared.
    unsafe {
        assert!(page.free().0.is_none());
        assert!(page.local_free().0.is_none());
    }

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: writes target the stack-local
    // page fixture to exercise typed atomic field accessors.
    unsafe {
        ptr::write(ptr::addr_of_mut!((*raw_page).xthread_id), 0x1000);
    }
    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: the thread-id field was
    // initialized in the stack-local page fixture above.
    unsafe {
        assert_eq!(page.thread_id_cell().load().raw(), 0x1000);
    }

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: writes target the stack-local
    // page fixture to exercise typed remote-free field accessors.
    unsafe {
        ptr::write(ptr::addr_of_mut!((*raw_page).xthread_free), 1);
    }
    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: the remote-free field was
    // initialized in the stack-local page fixture above.
    unsafe {
        assert_eq!(page.thread_free_cell().load_relaxed().raw(), 1);
    }

    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: key writes mutate only the
    // stack-local page fixture.
    unsafe { page.set_keys(PageKeys([11, 13])) };
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    let expected_keys = PageKeys([11, 13]);
    #[cfg(not(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    )))]
    let expected_keys = PageKeys::disabled();
    // SAFETY[AUDIT:typed-page-pointer-test-fixtures]: the page fixture remains live
    // and `keys` reads only the local metadata field.
    assert_eq!(unsafe { page.keys() }, expected_keys);
}

#[test]
fn block_ptr_free_list_link_helpers_delegate_to_encoded_links() {
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY[AUDIT:typed-block-pointer-test-fixtures]: the page pointer wraps
    // stack-local page storage used only by this free-list fixture.
    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };
    // SAFETY[AUDIT:typed-block-pointer-test-fixtures]: key writes mutate only the
    // stack-local page fixture and avoid encoded-key randomness in this test.
    unsafe { page.set_keys(PageKeys::disabled()) };

    let mut first = MaybeUninit::<mi_block_t>::zeroed();
    let mut second = MaybeUninit::<mi_block_t>::zeroed();
    // SAFETY[AUDIT:typed-block-pointer-test-fixtures]: both block pointers wrap
    // stack-local block fixtures that stay live through the link assertions.
    let first = unsafe { BlockPtr::new_unchecked(first.as_mut_ptr()) };
    // SAFETY[AUDIT:typed-block-pointer-test-fixtures]: `second` wraps stack-local
    // block storage used only as an encoded next pointer target.
    let second = unsafe { BlockPtr::new_unchecked(second.as_mut_ptr()) };

    // SAFETY[AUDIT:typed-block-pointer-test-fixtures]: link writes target stack-local
    // block metadata using the live stack-local page key fixture.
    unsafe { first.set_next(page, Some(second)) };
    // SAFETY[AUDIT:typed-block-pointer-test-fixtures]: the encoded link was written
    // above and both block fixtures remain live.
    unsafe {
        assert_eq!(
            first.next(page).map(BlockPtr::as_ptr),
            Some(second.as_ptr())
        );
    }

    // SAFETY[AUDIT:typed-block-pointer-test-fixtures]: link clearing mutates only the
    // stack-local first block fixture.
    unsafe { first.set_next(page, None) };
    // SAFETY[AUDIT:typed-block-pointer-test-fixtures]: the encoded link was just
    // cleared and the first block fixture remains live.
    unsafe {
        assert!(first.next(page).is_none());
    }
}

#[test]
fn heap_ptr_field_helpers_read_and_write_heap_fields() {
    let mut heap_storage = MaybeUninit::<mi_heap_t>::zeroed();
    let raw_heap = heap_storage.as_mut_ptr();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: `raw_heap` points at
    // stack-local heap storage that remains live for this test.
    let heap = unsafe { HeapPtr::new_unchecked(raw_heap) };

    let mut subproc_storage = MaybeUninit::<mi_subproc_t>::zeroed();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local subproc
    // storage used only as a heap field value.
    let subproc = unsafe { SubprocPtr::new_unchecked(subproc_storage.as_mut_ptr()) };
    let mut theap_storage = MaybeUninit::<mi_theap_t>::zeroed();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local thread-heap
    // storage used only as a heap field value.
    let theap = unsafe { ThreadHeapPtr::new_unchecked(theap_storage.as_mut_ptr()) };
    let mut arena_storage = MaybeUninit::<mi_arena_t>::zeroed();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local arena
    // storage used only as a heap field value.
    let arena = unsafe { ArenaPtr::new_unchecked(arena_storage.as_mut_ptr()) };

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: field writes mutate only the
    // stack-local heap fixture and store other live stack-local fixtures.
    unsafe {
        heap.set_subproc(Some(subproc));
        heap.set_thread_local_key(17);
        heap.set_thread_heaps(Some(theap));
        heap.set_exclusive_arena(Some(arena));
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: the heap fixture and stored
    // stack-local pointer values remain live through these field reads.
    unsafe {
        assert_eq!(
            heap.subproc().map(SubprocPtr::as_ptr),
            Some(subproc.as_ptr())
        );
        assert_eq!(heap.thread_local_key(), 17);
        assert_eq!(
            heap.thread_heaps().map(ThreadHeapPtr::as_ptr),
            Some(theap.as_ptr())
        );
        assert_eq!(
            heap.exclusive_arena().map(ArenaPtr::as_ptr),
            Some(arena.as_ptr())
        );
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: clearing fields mutates only
    // the stack-local heap fixture.
    unsafe {
        heap.set_subproc(None);
        heap.set_thread_heaps(None);
        heap.set_exclusive_arena(None);
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: the heap fixture remains live
    // and the nullable fields were just cleared.
    unsafe {
        assert!(heap.subproc().is_none());
        assert!(heap.thread_heaps().is_none());
        assert!(heap.exclusive_arena().is_none());
    }

    let arena_index = ArenaIndex::new(0).unwrap();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: raw field addresses are taken
    // from the same live stack-local heap fixture wrapped by `heap`.
    unsafe {
        assert_eq!(
            heap.arena_pages_cell(arena_index),
            ptr::addr_of_mut!((*raw_heap).arena_pages[0])
        );
        assert_eq!(
            heap.thread_heaps_lock(),
            ptr::addr_of_mut!((*raw_heap).theaps_lock)
        );
        assert_eq!(
            heap.arena_pages_lock(),
            ptr::addr_of_mut!((*raw_heap).arena_pages_lock)
        );
        assert_eq!(heap.stats(), ptr::addr_of_mut!((*raw_heap).stats));
    }
}

#[test]
fn thread_heap_ptr_field_helpers_read_and_write_thread_heap_fields() {
    let mut theap_storage = MaybeUninit::<mi_theap_t>::zeroed();
    let raw_theap = theap_storage.as_mut_ptr();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: `raw_theap` points at
    // stack-local thread-heap storage that remains live for this test.
    let theap = unsafe { ThreadHeapPtr::new_unchecked(raw_theap) };

    let mut tld_storage = MaybeUninit::<mi_tld_t>::zeroed();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local TLD storage
    // used only as a thread-heap field value.
    let tld = unsafe { TldPtr::new_unchecked(tld_storage.as_mut_ptr()) };
    let mut heap_storage = MaybeUninit::<mi_heap_t>::zeroed();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local heap storage
    // used only as a thread-heap field value.
    let heap = unsafe { HeapPtr::new_unchecked(heap_storage.as_mut_ptr()) };

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: field writes mutate only the
    // stack-local thread-heap fixture and store live stack-local pointers.
    unsafe {
        theap.set_tld(Some(tld));
        theap.set_heap(Some(heap));
        theap.set_heartbeat(Heartbeat::new(23));
        theap.refcount_cell().store_seq_cst(1);
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: reads target the same
    // stack-local thread-heap fixture and stored stack-local pointer values.
    unsafe {
        assert_eq!(theap.tld().map(TldPtr::as_ptr), Some(tld.as_ptr()));
        assert_eq!(theap.heap().map(HeapPtr::as_ptr), Some(heap.as_ptr()));
        assert_eq!(theap.heartbeat().get(), 23);
        assert_eq!((*raw_theap).refcount, 1);
        assert_eq!(theap.heap_cell().0, ptr::addr_of_mut!((*raw_theap).heap));
    }

    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local page storage
    // used only as a direct-page table value.
    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };
    let direct_bin = DirectBin::new(0).unwrap();

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: direct-page writes mutate only
    // the stack-local thread-heap fixture.
    unsafe { theap.set_direct_page(direct_bin, Some(page)) };
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: the stored page pointer refers
    // to the live stack-local page fixture assigned above.
    unsafe {
        assert_eq!(
            theap.direct_page(direct_bin).map(PagePtr::as_ptr),
            Some(page.as_ptr())
        );
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: direct-page clearing mutates
    // only the stack-local thread-heap fixture.
    unsafe { theap.set_direct_page(direct_bin, None) };
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: the direct-page slot was just
    // cleared and the thread-heap fixture remains live.
    unsafe {
        assert!(theap.direct_page(direct_bin).is_none());
    }

    let bin = Bin::new(0).unwrap();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: raw field addresses are taken
    // from the same live stack-local thread-heap fixture wrapped by `theap`.
    unsafe {
        assert_eq!(
            theap.page_queue(bin).as_ptr(),
            ptr::addr_of_mut!((*raw_theap).pages[0])
        );
        assert_eq!(theap.stats(), ptr::addr_of_mut!((*raw_theap).stats));
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: clearing nullable fields
    // mutates only the stack-local thread-heap fixture.
    unsafe {
        theap.set_tld(None);
        theap.set_heap(None);
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: the nullable fields were just
    // cleared and the thread-heap fixture remains live.
    unsafe {
        assert!(theap.tld().is_none());
        assert!(theap.heap().is_none());
    }
}

#[test]
fn tld_ptr_field_helpers_read_and_write_thread_local_fields() {
    let mut raw_tld = MaybeUninit::<mi_tld_t>::zeroed();
    let raw_tld = raw_tld.as_mut_ptr();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: `raw_tld` points at
    // stack-local TLD storage that remains live for this test.
    let tld = unsafe { TldPtr::new_unchecked(raw_tld) };

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: `0x1000` is a non-zero
    // thread-id fixture with page flag bits clear.
    let thread_id = unsafe { ThreadId::new_unchecked(0x1000) };
    let mut raw_subproc = MaybeUninit::<mi_subproc_t>::zeroed();
    let raw_subproc = raw_subproc.as_mut_ptr();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local subproc
    // storage used only as a TLD field value.
    let subproc = unsafe { SubprocPtr::new_unchecked(raw_subproc) };
    let mut raw_theap = MaybeUninit::<mi_theap_t>::zeroed();
    let raw_theap = raw_theap.as_mut_ptr();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local thread-heap
    // storage used only as a TLD field value.
    let theap = unsafe { ThreadHeapPtr::new_unchecked(raw_theap) };

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: field writes mutate only the
    // stack-local TLD fixture and store live stack-local pointer values.
    unsafe {
        tld.set_thread_id(Some(thread_id));
        tld.set_thread_seq(ThreadSeq::new(19));
        tld.set_numa_node(NumaNode::new(3));
        tld.set_subproc(Some(subproc));
        tld.set_thread_heaps(Some(theap));
        tld.set_recurse(true);
        tld.set_is_in_threadpool(true);
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: reads target the same live
    // stack-local TLD/subproc/thread-heap fixtures assigned above.
    unsafe {
        assert_eq!(tld.thread_id().map(ThreadId::get), Some(0x1000));
        assert_eq!(tld.thread_seq().get(), 19);
        assert_eq!(tld.numa_node().get(), 3);
        assert_eq!(
            tld.subproc().map(SubprocPtr::as_ptr),
            Some(subproc.as_ptr())
        );
        assert_eq!(tld.stats(), Some(ptr::addr_of_mut!((*raw_subproc).stats)));
        assert_eq!(
            tld.arena_count_cell().map(|cell| cell.0),
            Some(ptr::addr_of_mut!((*raw_subproc).arena_count).cast::<usize>())
        );
        assert_eq!(
            tld.thread_heaps().map(ThreadHeapPtr::as_ptr),
            Some(theap.as_ptr())
        );
        assert!(tld.recurse());
        assert!(tld.is_in_threadpool());
        assert_eq!(
            tld.thread_heaps_lock(),
            ptr::addr_of_mut!((*raw_tld).theaps_lock)
        );
        assert_eq!(tld.memid_ptr(), ptr::addr_of_mut!((*raw_tld).memid));
    }

    let mut raw_arena = MaybeUninit::<mi_arena_t>::zeroed();
    let raw_arena = raw_arena.as_mut_ptr();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: wraps stack-local arena
    // storage used only as a TLD arena-table value.
    let arena = unsafe { ArenaPtr::new_unchecked(raw_arena) };
    let arena_index = ArenaIndex::new(1).unwrap();
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: arena-table writes mutate only
    // the stack-local TLD fixture and store a live stack-local arena pointer.
    unsafe { tld.set_arena(arena_index, Some(arena)) };
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: the stored arena pointer refers
    // to the live stack-local arena fixture assigned above.
    unsafe {
        assert_eq!(
            tld.arena(arena_index).map(ArenaPtr::as_ptr),
            Some(arena.as_ptr())
        );
    }
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: arena-table clearing mutates
    // only the stack-local TLD fixture.
    unsafe { tld.set_arena(arena_index, None) };
    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: the arena slot was just
    // cleared and the TLD fixture remains live.
    unsafe {
        assert!(tld.arena(arena_index).is_none());
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: clearing nullable and boolean
    // fields mutates only the stack-local TLD fixture.
    unsafe {
        tld.set_thread_id(None);
        tld.set_subproc(None);
        tld.set_thread_heaps(None);
        tld.set_recurse(false);
        tld.set_is_in_threadpool(false);
    }

    // SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]: the TLD fixture remains live
    // and the nullable/boolean fields were just cleared.
    unsafe {
        assert!(tld.thread_id().is_none());
        assert!(tld.subproc().is_none());
        assert!(tld.stats().is_none());
        assert!(tld.arena_count_cell().is_none());
        assert!(tld.arena(arena_index).is_none());
        assert!(tld.thread_heaps().is_none());
        assert!(!tld.recurse());
        assert!(!tld.is_in_threadpool());
    }
}

#[test]
fn arena_ptr_field_helpers_read_and_write_arena_fields() {
    let mut raw_arena = MaybeUninit::<mi_arena_t>::zeroed();
    let raw_arena = raw_arena.as_mut_ptr();
    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: `raw_arena` points at
    // stack-local arena storage that remains live for this test.
    let arena = unsafe { ArenaPtr::new_unchecked(raw_arena) };

    let mut raw_subproc = MaybeUninit::<mi_subproc_t>::zeroed();
    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: wraps stack-local subproc
    // storage used only as an arena field value.
    let subproc = unsafe { SubprocPtr::new_unchecked(raw_subproc.as_mut_ptr()) };
    let mut raw_parent = MaybeUninit::<mi_arena_t>::zeroed();
    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: wraps stack-local parent arena
    // storage used only as an arena field value.
    let parent = unsafe { ArenaPtr::new_unchecked(raw_parent.as_mut_ptr()) };
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: wraps stack-local page storage
    // used only as an arena field value.
    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: field writes mutate only the
    // stack-local arena fixture and store live stack-local pointer values.
    unsafe {
        arena.set_subproc(Some(subproc));
        arena.set_index(ArenaIndex::new(2).unwrap());
        arena.set_slice_count(SliceCount::from_usize(5).unwrap());
        arena.set_numa_node(NumaNode::new(4));
        arena.set_is_exclusive(true);
        arena.set_parent(Some(parent));
        arena.set_pages_meta(Some(page));
    }

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: reads target the same live
    // stack-local arena/subproc/page fixtures assigned above.
    unsafe {
        assert_eq!(
            arena.subproc().map(SubprocPtr::as_ptr),
            Some(subproc.as_ptr())
        );
        assert_eq!(arena.index().get(), 2);
        assert_eq!(arena.slice_count().get(), 5);
        assert_eq!(arena.numa_node().get(), 4);
        assert!(arena.is_exclusive());
        assert_eq!(arena.parent().map(ArenaPtr::as_ptr), Some(parent.as_ptr()));
        assert_eq!(arena.pages_meta().map(PagePtr::as_ptr), Some(page.as_ptr()));
    }

    let mut slices_free = MaybeUninit::<mi_bbitmap_t>::zeroed();
    let mut slices_committed = MaybeUninit::<mi_bitmap_t>::zeroed();
    let mut slices_dirty = MaybeUninit::<mi_bitmap_t>::zeroed();
    let mut slices_purge = MaybeUninit::<mi_bitmap_t>::zeroed();

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: bitmap pointer writes mutate
    // only the stack-local arena fixture and store live stack-local bitmaps.
    unsafe {
        ptr::write(
            ptr::addr_of_mut!((*raw_arena).slices_free),
            slices_free.as_mut_ptr(),
        );
        ptr::write(
            ptr::addr_of_mut!((*raw_arena).slices_committed),
            slices_committed.as_mut_ptr(),
        );
        ptr::write(
            ptr::addr_of_mut!((*raw_arena).slices_dirty),
            slices_dirty.as_mut_ptr(),
        );
        ptr::write(
            ptr::addr_of_mut!((*raw_arena).slices_purge),
            slices_purge.as_mut_ptr(),
        );
    }

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: raw field addresses are taken
    // from the same live stack-local arena fixture wrapped by `arena`.
    unsafe {
        assert_eq!(arena.slices_free(), slices_free.as_mut_ptr());
        assert_eq!(arena.slices_committed(), slices_committed.as_mut_ptr());
        assert_eq!(arena.slices_dirty(), slices_dirty.as_mut_ptr());
        assert_eq!(arena.slices_purge(), slices_purge.as_mut_ptr());
        assert_eq!(arena.memid_ptr(), ptr::addr_of_mut!((*raw_arena).memid));
        assert_eq!(
            arena.purge_expire_cell().0,
            ptr::addr_of_mut!((*raw_arena).purge_expire).cast::<i64>()
        );
        assert_eq!(
            arena.pages_main(),
            ptr::addr_of_mut!((*raw_arena).pages_main)
        );
    }

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: clearing fields mutates only
    // the stack-local arena fixture.
    unsafe {
        arena.set_subproc(None);
        arena.set_parent(None);
        arena.set_pages_meta(None);
        arena.set_is_exclusive(false);
    }

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: the arena fixture remains live
    // and the nullable/boolean fields were just cleared.
    unsafe {
        assert!(arena.subproc().is_none());
        assert!(arena.parent().is_none());
        assert!(arena.pages_meta().is_none());
        assert!(!arena.is_exclusive());
    }
}

#[test]
fn subproc_ptr_field_helpers_read_and_write_subproc_fields() {
    let mut raw_subproc = MaybeUninit::<mi_subproc_t>::zeroed();
    let raw_subproc = raw_subproc.as_mut_ptr();
    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: `raw_subproc` points at
    // stack-local subproc storage that remains live for this test.
    let subproc = unsafe { SubprocPtr::new_unchecked(raw_subproc) };

    let mut raw_arena = MaybeUninit::<mi_arena_t>::zeroed();
    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: wraps stack-local arena
    // storage used only as a subproc arena-table value.
    let arena = unsafe { ArenaPtr::new_unchecked(raw_arena.as_mut_ptr()) };
    let mut raw_heap = MaybeUninit::<mi_heap_t>::zeroed();
    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: wraps stack-local heap storage
    // used only as subproc heap-list and main-heap values.
    let heap = unsafe { HeapPtr::new_unchecked(raw_heap.as_mut_ptr()) };
    let arena_index = ArenaIndex::new(1).unwrap();

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: field writes mutate only the
    // stack-local subproc fixture and store live stack-local pointers.
    unsafe {
        subproc.set_sequence(SubprocSeq::new(29));
        subproc.set_arena_count(ArenaCount::new(2), MemoryOrder::Relaxed);
        subproc.set_thread_count(ThreadCount::new(3), MemoryOrder::Relaxed);
        subproc.set_thread_total_count(ThreadCount::new(5), MemoryOrder::Relaxed);
        subproc.set_heap_count(HeapCount::new(7), MemoryOrder::Relaxed);
        subproc.set_heap_total_count(HeapCount::new(11), MemoryOrder::Relaxed);
        subproc.set_arena(arena_index, Some(arena));
        subproc.set_main_heap(Some(heap));
        subproc.set_heaps(Some(heap));
    }

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: reads and raw field addresses
    // target the same live stack-local subproc/arena/heap fixtures.
    unsafe {
        assert_eq!(subproc.sequence().get(), 29);
        assert_eq!(subproc.arena_count(MemoryOrder::Relaxed).get(), 2);
        assert_eq!(subproc.thread_count(MemoryOrder::Relaxed).get(), 3);
        assert_eq!(subproc.thread_total_count(MemoryOrder::Relaxed).get(), 5);
        assert_eq!(subproc.heap_count(MemoryOrder::Relaxed).get(), 7);
        assert_eq!(subproc.heap_total_count(MemoryOrder::Relaxed).get(), 11);
        assert_eq!(
            subproc.arena(arena_index).map(ArenaPtr::as_ptr),
            Some(arena.as_ptr())
        );
        assert_eq!(
            subproc.main_heap().map(HeapPtr::as_ptr),
            Some(heap.as_ptr())
        );
        assert_eq!(subproc.heaps().map(HeapPtr::as_ptr), Some(heap.as_ptr()));
        assert_eq!(
            subproc.arena_count_cell().0,
            ptr::addr_of_mut!((*raw_subproc).arena_count).cast::<usize>()
        );
        assert_eq!(
            subproc.thread_count_cell().0,
            ptr::addr_of_mut!((*raw_subproc).thread_count).cast::<usize>()
        );
        assert_eq!(
            subproc.thread_total_count_cell().0,
            ptr::addr_of_mut!((*raw_subproc).thread_total_count).cast::<usize>()
        );
        assert_eq!(
            subproc.heap_count_cell().0,
            ptr::addr_of_mut!((*raw_subproc).heap_count).cast::<usize>()
        );
        assert_eq!(
            subproc.heap_total_count_cell().0,
            ptr::addr_of_mut!((*raw_subproc).heap_total_count).cast::<usize>()
        );
        assert_eq!(
            subproc.purge_expire_cell().0,
            ptr::addr_of_mut!((*raw_subproc).purge_expire).cast::<i64>()
        );
        assert_eq!(
            subproc.arena_reserve_lock().0.as_ptr(),
            ptr::addr_of_mut!((*raw_subproc).arena_reserve_lock)
        );
        assert_eq!(
            subproc.heaps_lock().0.as_ptr(),
            ptr::addr_of_mut!((*raw_subproc).heaps_lock)
        );
        assert_eq!(subproc.memid_ptr(), ptr::addr_of_mut!((*raw_subproc).memid));
        assert_eq!(subproc.stats(), ptr::addr_of_mut!((*raw_subproc).stats));
    }

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: clearing nullable fields
    // mutates only the stack-local subproc fixture.
    unsafe {
        subproc.set_arena(arena_index, None);
        subproc.set_main_heap(None);
        subproc.set_heaps(None);
    }

    // SAFETY[AUDIT:typed-arena-subproc-pointer-test-fixtures]: the nullable fields were just
    // cleared and the subproc fixture remains live.
    unsafe {
        assert!(subproc.arena(arena_index).is_none());
        assert!(subproc.main_heap().is_none());
        assert!(subproc.heaps().is_none());
    }
}

#[test]
fn page_queue_ptr_field_helpers_read_and_write_queue_fields() {
    let mut raw_queue = MaybeUninit::<mi_page_queue_t>::zeroed();
    let raw_queue = raw_queue.as_mut_ptr();
    // SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]: `raw_queue` points at
    // stack-local page-queue storage that remains live for this test.
    let queue = unsafe { PageQueuePtr::new_unchecked(raw_queue) };

    let mut raw_first = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]: wraps stack-local page storage
    // used only as the queue first-page value.
    let first = unsafe { PagePtr::new_unchecked(raw_first.as_mut_ptr()) };
    let mut raw_last = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]: wraps stack-local page storage
    // used only as the queue last-page value.
    let last = unsafe { PagePtr::new_unchecked(raw_last.as_mut_ptr()) };

    // SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]: field writes mutate only the
    // stack-local queue fixture and store live stack-local page pointers.
    unsafe {
        queue.set_first(Some(first));
        queue.set_last(Some(last));
        queue.set_count(PageQueueCount::new(3));
        queue.set_block_size(Some(BlockSize::new_unchecked(64)));
    }

    // SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]: reads target the same live
    // stack-local queue/page fixtures assigned above.
    unsafe {
        assert_eq!(queue.first().map(PagePtr::as_ptr), Some(first.as_ptr()));
        assert_eq!(queue.last().map(PagePtr::as_ptr), Some(last.as_ptr()));
        assert_eq!(queue.count().get(), 3);
        assert_eq!(queue.block_size().map(BlockSize::get), Some(64));
    }

    // SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]: clearing fields mutates only
    // the stack-local queue fixture.
    unsafe {
        queue.set_first(None);
        queue.set_last(None);
        queue.set_count(PageQueueCount::new(0));
        queue.set_block_size(None);
    }

    // SAFETY[AUDIT:typed-page-queue-pointer-test-fixtures]: the queue fixture remains live
    // and the fields were just cleared.
    unsafe {
        assert!(queue.first().is_none());
        assert!(queue.last().is_none());
        assert_eq!(queue.count().get(), 0);
        assert!(queue.block_size().is_none());
    }
}

#[test]
fn user_ptr_conversion_helpers_preserve_pointer_values() {
    let mut byte = 0u8;
    let raw = (&mut byte as *mut u8).cast::<c_void>();
    let non_null = NonNull::new(raw).unwrap();

    let user = UserPtr::from_non_null(non_null);
    assert_eq!(user.as_ptr(), raw);
    assert_eq!(user.as_const_ptr(), raw.cast_const());
    assert_eq!(user.as_u8_ptr(), &mut byte as *mut u8);
    assert_eq!(user.as_non_null(), non_null);
    assert_eq!(UserPtr::option_as_ptr(Some(user)), raw);
    assert_eq!(UserPtr::option_as_ptr(None), ptr::null_mut());

    let interior = user.into_interior();
    assert_eq!(interior.as_ptr(), raw);
    assert_eq!(interior.as_const_ptr(), raw.cast_const());
    assert_eq!(interior.as_u8_ptr(), &mut byte as *mut u8);
    assert_eq!(interior.as_non_null(), non_null);
    assert_eq!(InteriorUserPtr::option_as_ptr(Some(interior)), raw);
    assert_eq!(InteriorUserPtr::option_as_ptr(None), ptr::null_mut());

    let roundtrip = interior.as_user_ptr();
    assert_eq!(roundtrip.as_ptr(), raw);
    assert_eq!(InteriorUserPtr::from_non_null(non_null).as_ptr(), raw);
}

#[test]
fn start_and_os_pointer_checked_offset_helpers_preserve_values() {
    let mut bytes = [0u8; 16];
    let base = bytes.as_mut_ptr();
    // SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]: `base` is a non-null pointer
    // into a live stack-local byte buffer.
    let page_start = unsafe { PageStart::new_unchecked(base) };
    // SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]: `base` is a non-null pointer
    // into a live stack-local byte buffer.
    let slice_start = unsafe { SliceStart::new_unchecked(base) };
    let os_mem = OsMemPtr::from_non_null(NonNull::new(base.cast::<c_void>()).unwrap());

    assert_eq!(page_start.as_non_null().as_ptr(), base);
    assert_eq!(slice_start.as_non_null().as_ptr(), base);
    assert_eq!(os_mem.as_non_null().as_ptr(), base.cast::<c_void>());
    assert_eq!(os_mem.as_u8_non_null().as_ptr(), base);

    assert_eq!(page_start.checked_byte_add(0).unwrap().as_ptr(), base);
    assert_eq!(slice_start.checked_byte_add(0).unwrap().as_ptr(), base);
    assert_eq!(
        os_mem.checked_byte_add(0).unwrap().as_ptr(),
        base.cast::<c_void>()
    );

    let page_at_7 = page_start.checked_byte_add(7).unwrap();
    let slice_at_9 = slice_start.checked_byte_add(9).unwrap();
    let os_at_11 = os_mem.checked_byte_add(11).unwrap();

    // SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]: offsets 7, 9, and 11 are
    // within the live 16-byte stack buffer.
    unsafe {
        assert_eq!(page_at_7.as_ptr(), base.add(7));
        assert_eq!(slice_at_9.as_ptr(), base.add(9));
        assert_eq!(os_at_11.as_ptr(), base.add(11).cast::<c_void>());
    }
    assert_eq!(page_start.checked_byte_sub(0).unwrap().as_ptr(), base);
    assert_eq!(slice_start.checked_byte_sub(0).unwrap().as_ptr(), base);
    assert_eq!(
        os_at_11.checked_byte_sub(11).unwrap().as_ptr(),
        base.cast::<c_void>()
    );

    #[cfg(not(miri))]
    {
        // SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]: this non-dereferenced
        // high-address fixture is used only to exercise checked overflow.
        let high = unsafe { PageStart::new_unchecked(usize::MAX as *mut u8) };
        assert!(high.checked_byte_add(1).is_none());
    }
    // SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]: this non-dereferenced
    // dangling fixture is used only to exercise checked underflow.
    let low = unsafe { SliceStart::new_unchecked(ptr::dangling_mut::<u8>()) };
    assert!(low.checked_byte_sub(2).is_none());
    #[cfg(not(miri))]
    {
        // SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]: this non-dereferenced
        // high-address fixture is used only to exercise checked overflow.
        let high_os = unsafe { OsMemPtr::new_unchecked(usize::MAX as *mut c_void) };
        assert!(high_os.checked_byte_add(1).is_none());
    }
}

#[test]
fn provenance_address_helpers_match_raw_pointer_math() {
    let bytes = [0u8; 16];
    let base = bytes.as_ptr();
    // SAFETY[AUDIT:typed-provenance-pointer-test-fixtures]: offset 9 is within the live
    // stack-local byte buffer and is used only for address-difference checks.
    let at_9 = unsafe { base.add(9) };

    assert_eq!(ptr_exposed_addr(base), base as usize);
    assert_eq!(ptr_addr_diff(at_9, base), 9);
    assert!(ptr_is_aligned(base, 1));
    assert!(ptr_is_aligned(base, 0));
}
