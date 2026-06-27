use super::*;
use core::mem::MaybeUninit;

#[test]
fn page_ptr_field_helpers_read_and_write_hot_fields() {
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    let raw_page = raw_page.as_mut_ptr();
    let page = unsafe { PagePtr::new_unchecked(raw_page) };

    unsafe {
        page.set_block_size(BlockSize::new_unchecked(64));
        page.set_used(UsedBlockCount::new(3));
        ptr::write(ptr::addr_of_mut!((*raw_page).reserved), 7);
        ptr::write(ptr::addr_of_mut!((*raw_page).capacity), 5);
    }

    assert_eq!(unsafe { page.block_size().get() }, 64);
    assert_eq!(unsafe { page.used().get() }, 3);
    assert_eq!(unsafe { page.capacity().get() }, 5);
    assert_eq!(unsafe { page.reserved().get() }, 7);

    let mut free_block = MaybeUninit::<mi_block_t>::zeroed();
    let mut local_block = MaybeUninit::<mi_block_t>::zeroed();
    let free_head = free_block.as_mut_ptr();
    let local_head = local_block.as_mut_ptr();

    unsafe {
        page.set_free(AvailableFreeList::from_raw(free_head));
        page.set_local_free(LocalFreeList::from_raw(local_head));
    }

    assert_eq!(
        unsafe { page.free().0.map(BlockPtr::as_ptr) },
        Some(free_head)
    );
    assert_eq!(
        unsafe { page.local_free().0.map(BlockPtr::as_ptr) },
        Some(local_head)
    );

    unsafe {
        page.set_free(AvailableFreeList::from_raw(ptr::null_mut()));
        page.set_local_free(LocalFreeList::from_raw(ptr::null_mut()));
    }

    assert!(unsafe { page.free().0.is_none() });
    assert!(unsafe { page.local_free().0.is_none() });

    unsafe {
        ptr::write(ptr::addr_of_mut!((*raw_page).xthread_id), 0x1000);
    }
    assert_eq!(unsafe { page.thread_id_cell().load().raw() }, 0x1000);

    unsafe {
        ptr::write(ptr::addr_of_mut!((*raw_page).xthread_free), 1);
    }
    assert_eq!(unsafe { page.thread_free_cell().load_relaxed().raw() }, 1);

    unsafe { page.set_keys(PageKeys([11, 13])) };
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    assert_eq!(unsafe { page.keys() }, PageKeys([11, 13]));
    #[cfg(not(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    )))]
    assert_eq!(unsafe { page.keys() }, PageKeys::disabled());
}

#[test]
fn block_ptr_free_list_link_helpers_delegate_to_encoded_links() {
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };
    unsafe { page.set_keys(PageKeys::disabled()) };

    let mut first = MaybeUninit::<mi_block_t>::zeroed();
    let mut second = MaybeUninit::<mi_block_t>::zeroed();
    let first = unsafe { BlockPtr::new_unchecked(first.as_mut_ptr()) };
    let second = unsafe { BlockPtr::new_unchecked(second.as_mut_ptr()) };

    unsafe { first.set_next(page, Some(second)) };
    assert_eq!(
        unsafe { first.next(page).map(BlockPtr::as_ptr) },
        Some(second.as_ptr())
    );

    unsafe { first.set_next(page, None) };
    assert!(unsafe { first.next(page).is_none() });
}

#[test]
fn heap_ptr_field_helpers_read_and_write_heap_fields() {
    let mut heap_storage = MaybeUninit::<mi_heap_t>::zeroed();
    let raw_heap = heap_storage.as_mut_ptr();
    let heap = unsafe { HeapPtr::new_unchecked(raw_heap) };

    let mut subproc_storage = MaybeUninit::<mi_subproc_t>::zeroed();
    let subproc = unsafe { SubprocPtr::new_unchecked(subproc_storage.as_mut_ptr()) };
    let mut theap_storage = MaybeUninit::<mi_theap_t>::zeroed();
    let theap = unsafe { ThreadHeapPtr::new_unchecked(theap_storage.as_mut_ptr()) };
    let mut arena_storage = MaybeUninit::<mi_arena_t>::zeroed();
    let arena = unsafe { ArenaPtr::new_unchecked(arena_storage.as_mut_ptr()) };

    unsafe {
        heap.set_subproc(Some(subproc));
        heap.set_thread_local_key(17);
        heap.set_thread_heaps(Some(theap));
        heap.set_exclusive_arena(Some(arena));
    }

    assert_eq!(
        unsafe { heap.subproc().map(SubprocPtr::as_ptr) },
        Some(subproc.as_ptr())
    );
    assert_eq!(unsafe { heap.thread_local_key() }, 17);
    assert_eq!(
        unsafe { heap.thread_heaps().map(ThreadHeapPtr::as_ptr) },
        Some(theap.as_ptr())
    );
    assert_eq!(
        unsafe { heap.exclusive_arena().map(ArenaPtr::as_ptr) },
        Some(arena.as_ptr())
    );

    unsafe {
        heap.set_subproc(None);
        heap.set_thread_heaps(None);
        heap.set_exclusive_arena(None);
    }

    assert!(unsafe { heap.subproc().is_none() });
    assert!(unsafe { heap.thread_heaps().is_none() });
    assert!(unsafe { heap.exclusive_arena().is_none() });

    let arena_index = ArenaIndex::new(0).unwrap();
    assert_eq!(unsafe { heap.arena_pages_cell(arena_index) }, unsafe {
        ptr::addr_of_mut!((*raw_heap).arena_pages[0])
    });
    assert_eq!(unsafe { heap.thread_heaps_lock() }, unsafe {
        ptr::addr_of_mut!((*raw_heap).theaps_lock)
    });
    assert_eq!(unsafe { heap.arena_pages_lock() }, unsafe {
        ptr::addr_of_mut!((*raw_heap).arena_pages_lock)
    });
    assert_eq!(unsafe { heap.stats() }, unsafe {
        ptr::addr_of_mut!((*raw_heap).stats)
    });
}

#[test]
fn thread_heap_ptr_field_helpers_read_and_write_thread_heap_fields() {
    let mut theap_storage = MaybeUninit::<mi_theap_t>::zeroed();
    let raw_theap = theap_storage.as_mut_ptr();
    let theap = unsafe { ThreadHeapPtr::new_unchecked(raw_theap) };

    let mut tld_storage = MaybeUninit::<mi_tld_t>::zeroed();
    let tld = unsafe { TldPtr::new_unchecked(tld_storage.as_mut_ptr()) };
    let mut heap_storage = MaybeUninit::<mi_heap_t>::zeroed();
    let heap = unsafe { HeapPtr::new_unchecked(heap_storage.as_mut_ptr()) };

    unsafe {
        theap.set_tld(Some(tld));
        theap.set_heap(Some(heap));
        theap.set_heartbeat(Heartbeat::new(23));
        theap.refcount_cell().store_seq_cst(1);
    }

    assert_eq!(
        unsafe { theap.tld().map(TldPtr::as_ptr) },
        Some(tld.as_ptr())
    );
    assert_eq!(
        unsafe { theap.heap().map(HeapPtr::as_ptr) },
        Some(heap.as_ptr())
    );
    assert_eq!(unsafe { theap.heartbeat().get() }, 23);
    assert_eq!(unsafe { (*raw_theap).refcount }, 1);
    assert_eq!(unsafe { theap.heap_cell().0 }, unsafe {
        ptr::addr_of_mut!((*raw_theap).heap)
    });

    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };
    let direct_bin = DirectBin::new(0).unwrap();

    unsafe { theap.set_direct_page(direct_bin, Some(page)) };
    assert_eq!(
        unsafe { theap.direct_page(direct_bin).map(PagePtr::as_ptr) },
        Some(page.as_ptr())
    );

    unsafe { theap.set_direct_page(direct_bin, None) };
    assert!(unsafe { theap.direct_page(direct_bin).is_none() });

    let bin = Bin::new(0).unwrap();
    assert_eq!(unsafe { theap.page_queue(bin).as_ptr() }, unsafe {
        ptr::addr_of_mut!((*raw_theap).pages[0])
    });
    assert_eq!(unsafe { theap.stats() }, unsafe {
        ptr::addr_of_mut!((*raw_theap).stats)
    });

    unsafe {
        theap.set_tld(None);
        theap.set_heap(None);
    }

    assert!(unsafe { theap.tld().is_none() });
    assert!(unsafe { theap.heap().is_none() });
}

#[test]
fn tld_ptr_field_helpers_read_and_write_thread_local_fields() {
    let mut raw_tld = MaybeUninit::<mi_tld_t>::zeroed();
    let raw_tld = raw_tld.as_mut_ptr();
    let tld = unsafe { TldPtr::new_unchecked(raw_tld) };

    let thread_id = unsafe { ThreadId::new_unchecked(0x1000) };
    let mut raw_subproc = MaybeUninit::<mi_subproc_t>::zeroed();
    let raw_subproc = raw_subproc.as_mut_ptr();
    let subproc = unsafe { SubprocPtr::new_unchecked(raw_subproc) };
    let mut raw_theap = MaybeUninit::<mi_theap_t>::zeroed();
    let raw_theap = raw_theap.as_mut_ptr();
    let theap = unsafe { ThreadHeapPtr::new_unchecked(raw_theap) };

    unsafe {
        tld.set_thread_id(Some(thread_id));
        tld.set_thread_seq(ThreadSeq::new(19));
        tld.set_numa_node(NumaNode::new(3));
        tld.set_subproc(Some(subproc));
        tld.set_thread_heaps(Some(theap));
        tld.set_recurse(true);
        tld.set_is_in_threadpool(true);
    }

    assert_eq!(unsafe { tld.thread_id().map(ThreadId::get) }, Some(0x1000));
    assert_eq!(unsafe { tld.thread_seq().get() }, 19);
    assert_eq!(unsafe { tld.numa_node().get() }, 3);
    assert_eq!(
        unsafe { tld.subproc().map(SubprocPtr::as_ptr) },
        Some(subproc.as_ptr())
    );
    assert_eq!(unsafe { tld.stats() }, unsafe {
        Some(ptr::addr_of_mut!((*raw_subproc).stats))
    });
    assert_eq!(
        unsafe { tld.arena_count_cell().map(|cell| cell.0) },
        unsafe { Some(ptr::addr_of_mut!((*raw_subproc).arena_count).cast::<usize>()) }
    );
    assert_eq!(
        unsafe { tld.thread_heaps().map(ThreadHeapPtr::as_ptr) },
        Some(theap.as_ptr())
    );
    assert!(unsafe { tld.recurse() });
    assert!(unsafe { tld.is_in_threadpool() });
    assert_eq!(unsafe { tld.thread_heaps_lock() }, unsafe {
        ptr::addr_of_mut!((*raw_tld).theaps_lock)
    });
    assert_eq!(unsafe { tld.memid_ptr() }, unsafe {
        ptr::addr_of_mut!((*raw_tld).memid)
    });

    let mut raw_arena = MaybeUninit::<mi_arena_t>::zeroed();
    let raw_arena = raw_arena.as_mut_ptr();
    let arena = unsafe { ArenaPtr::new_unchecked(raw_arena) };
    let arena_index = ArenaIndex::new(1).unwrap();
    unsafe { tld.set_arena(arena_index, Some(arena)) };
    assert_eq!(
        unsafe { tld.arena(arena_index).map(ArenaPtr::as_ptr) },
        Some(arena.as_ptr())
    );
    unsafe { tld.set_arena(arena_index, None) };
    assert!(unsafe { tld.arena(arena_index).is_none() });

    unsafe {
        tld.set_thread_id(None);
        tld.set_subproc(None);
        tld.set_thread_heaps(None);
        tld.set_recurse(false);
        tld.set_is_in_threadpool(false);
    }

    assert!(unsafe { tld.thread_id().is_none() });
    assert!(unsafe { tld.subproc().is_none() });
    assert!(unsafe { tld.stats().is_none() });
    assert!(unsafe { tld.arena_count_cell().is_none() });
    assert!(unsafe { tld.arena(arena_index).is_none() });
    assert!(unsafe { tld.thread_heaps().is_none() });
    assert!(!unsafe { tld.recurse() });
    assert!(!unsafe { tld.is_in_threadpool() });
}

#[test]
fn arena_ptr_field_helpers_read_and_write_arena_fields() {
    let mut raw_arena = MaybeUninit::<mi_arena_t>::zeroed();
    let raw_arena = raw_arena.as_mut_ptr();
    let arena = unsafe { ArenaPtr::new_unchecked(raw_arena) };

    let mut raw_subproc = MaybeUninit::<mi_subproc_t>::zeroed();
    let subproc = unsafe { SubprocPtr::new_unchecked(raw_subproc.as_mut_ptr()) };
    let mut raw_parent = MaybeUninit::<mi_arena_t>::zeroed();
    let parent = unsafe { ArenaPtr::new_unchecked(raw_parent.as_mut_ptr()) };
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };

    unsafe {
        arena.set_subproc(Some(subproc));
        arena.set_index(ArenaIndex::new(2).unwrap());
        arena.set_slice_count(SliceCount::from_usize(5).unwrap());
        arena.set_numa_node(NumaNode::new(4));
        arena.set_is_exclusive(true);
        arena.set_parent(Some(parent));
        arena.set_pages_meta(Some(page));
    }

    assert_eq!(
        unsafe { arena.subproc().map(SubprocPtr::as_ptr) },
        Some(subproc.as_ptr())
    );
    assert_eq!(unsafe { arena.index().get() }, 2);
    assert_eq!(unsafe { arena.slice_count().get() }, 5);
    assert_eq!(unsafe { arena.numa_node().get() }, 4);
    assert!(unsafe { arena.is_exclusive() });
    assert_eq!(
        unsafe { arena.parent().map(ArenaPtr::as_ptr) },
        Some(parent.as_ptr())
    );
    assert_eq!(
        unsafe { arena.pages_meta().map(PagePtr::as_ptr) },
        Some(page.as_ptr())
    );

    let mut slices_free = MaybeUninit::<mi_bbitmap_t>::zeroed();
    let mut slices_committed = MaybeUninit::<mi_bitmap_t>::zeroed();
    let mut slices_dirty = MaybeUninit::<mi_bitmap_t>::zeroed();
    let mut slices_purge = MaybeUninit::<mi_bitmap_t>::zeroed();

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

    assert_eq!(unsafe { arena.slices_free() }, slices_free.as_mut_ptr());
    assert_eq!(
        unsafe { arena.slices_committed() },
        slices_committed.as_mut_ptr()
    );
    assert_eq!(unsafe { arena.slices_dirty() }, slices_dirty.as_mut_ptr());
    assert_eq!(unsafe { arena.slices_purge() }, slices_purge.as_mut_ptr());

    assert_eq!(unsafe { arena.memid_ptr() }, unsafe {
        ptr::addr_of_mut!((*raw_arena).memid)
    });
    assert_eq!(unsafe { arena.purge_expire_cell().0 }, unsafe {
        ptr::addr_of_mut!((*raw_arena).purge_expire).cast::<i64>()
    });
    assert_eq!(unsafe { arena.pages_main() }, unsafe {
        ptr::addr_of_mut!((*raw_arena).pages_main)
    });

    unsafe {
        arena.set_subproc(None);
        arena.set_parent(None);
        arena.set_pages_meta(None);
        arena.set_is_exclusive(false);
    }

    assert!(unsafe { arena.subproc().is_none() });
    assert!(unsafe { arena.parent().is_none() });
    assert!(unsafe { arena.pages_meta().is_none() });
    assert!(!unsafe { arena.is_exclusive() });
}

#[test]
fn subproc_ptr_field_helpers_read_and_write_subproc_fields() {
    let mut raw_subproc = MaybeUninit::<mi_subproc_t>::zeroed();
    let raw_subproc = raw_subproc.as_mut_ptr();
    let subproc = unsafe { SubprocPtr::new_unchecked(raw_subproc) };

    let mut raw_arena = MaybeUninit::<mi_arena_t>::zeroed();
    let arena = unsafe { ArenaPtr::new_unchecked(raw_arena.as_mut_ptr()) };
    let mut raw_heap = MaybeUninit::<mi_heap_t>::zeroed();
    let heap = unsafe { HeapPtr::new_unchecked(raw_heap.as_mut_ptr()) };
    let arena_index = ArenaIndex::new(1).unwrap();

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

    assert_eq!(unsafe { subproc.sequence().get() }, 29);
    assert_eq!(
        unsafe { subproc.arena_count(MemoryOrder::Relaxed).get() },
        2
    );
    assert_eq!(
        unsafe { subproc.thread_count(MemoryOrder::Relaxed).get() },
        3
    );
    assert_eq!(
        unsafe { subproc.thread_total_count(MemoryOrder::Relaxed).get() },
        5
    );
    assert_eq!(unsafe { subproc.heap_count(MemoryOrder::Relaxed).get() }, 7);
    assert_eq!(
        unsafe { subproc.heap_total_count(MemoryOrder::Relaxed).get() },
        11
    );
    assert_eq!(
        unsafe { subproc.arena(arena_index).map(ArenaPtr::as_ptr) },
        Some(arena.as_ptr())
    );
    assert_eq!(
        unsafe { subproc.main_heap().map(HeapPtr::as_ptr) },
        Some(heap.as_ptr())
    );
    assert_eq!(
        unsafe { subproc.heaps().map(HeapPtr::as_ptr) },
        Some(heap.as_ptr())
    );

    assert_eq!(unsafe { subproc.arena_count_cell().0 }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).arena_count).cast::<usize>()
    });
    assert_eq!(unsafe { subproc.thread_count_cell().0 }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).thread_count).cast::<usize>()
    });
    assert_eq!(unsafe { subproc.thread_total_count_cell().0 }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).thread_total_count).cast::<usize>()
    });
    assert_eq!(unsafe { subproc.heap_count_cell().0 }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).heap_count).cast::<usize>()
    });
    assert_eq!(unsafe { subproc.heap_total_count_cell().0 }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).heap_total_count).cast::<usize>()
    });
    assert_eq!(unsafe { subproc.purge_expire_cell().0 }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).purge_expire).cast::<i64>()
    });
    assert_eq!(unsafe { subproc.arena_reserve_lock().0.as_ptr() }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).arena_reserve_lock)
    });
    assert_eq!(unsafe { subproc.heaps_lock().0.as_ptr() }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).heaps_lock)
    });
    assert_eq!(unsafe { subproc.memid_ptr() }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).memid)
    });
    assert_eq!(unsafe { subproc.stats() }, unsafe {
        ptr::addr_of_mut!((*raw_subproc).stats)
    });

    unsafe {
        subproc.set_arena(arena_index, None);
        subproc.set_main_heap(None);
        subproc.set_heaps(None);
    }

    assert!(unsafe { subproc.arena(arena_index).is_none() });
    assert!(unsafe { subproc.main_heap().is_none() });
    assert!(unsafe { subproc.heaps().is_none() });
}

#[test]
fn page_queue_ptr_field_helpers_read_and_write_queue_fields() {
    let mut raw_queue = MaybeUninit::<mi_page_queue_t>::zeroed();
    let raw_queue = raw_queue.as_mut_ptr();
    let queue = unsafe { PageQueuePtr::new_unchecked(raw_queue) };

    let mut raw_first = MaybeUninit::<mi_page_t>::zeroed();
    let first = unsafe { PagePtr::new_unchecked(raw_first.as_mut_ptr()) };
    let mut raw_last = MaybeUninit::<mi_page_t>::zeroed();
    let last = unsafe { PagePtr::new_unchecked(raw_last.as_mut_ptr()) };

    unsafe {
        queue.set_first(Some(first));
        queue.set_last(Some(last));
        queue.set_count(PageQueueCount::new(3));
        queue.set_block_size(Some(BlockSize::new_unchecked(64)));
    }

    assert_eq!(
        unsafe { queue.first().map(PagePtr::as_ptr) },
        Some(first.as_ptr())
    );
    assert_eq!(
        unsafe { queue.last().map(PagePtr::as_ptr) },
        Some(last.as_ptr())
    );
    assert_eq!(unsafe { queue.count().get() }, 3);
    assert_eq!(unsafe { queue.block_size().map(BlockSize::get) }, Some(64));

    unsafe {
        queue.set_first(None);
        queue.set_last(None);
        queue.set_count(PageQueueCount::new(0));
        queue.set_block_size(None);
    }

    assert!(unsafe { queue.first().is_none() });
    assert!(unsafe { queue.last().is_none() });
    assert_eq!(unsafe { queue.count().get() }, 0);
    assert!(unsafe { queue.block_size().is_none() });
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
    let page_start = unsafe { PageStart::new_unchecked(base) };
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

    assert_eq!(page_at_7.as_ptr(), unsafe { base.add(7) });
    assert_eq!(slice_at_9.as_ptr(), unsafe { base.add(9) });
    assert_eq!(os_at_11.as_ptr(), unsafe { base.add(11).cast::<c_void>() });
    assert_eq!(page_start.checked_byte_sub(0).unwrap().as_ptr(), base);
    assert_eq!(slice_start.checked_byte_sub(0).unwrap().as_ptr(), base);
    assert_eq!(
        os_at_11.checked_byte_sub(11).unwrap().as_ptr(),
        base.cast::<c_void>()
    );

    #[cfg(not(miri))]
    {
        let high = unsafe { PageStart::new_unchecked(usize::MAX as *mut u8) };
        assert!(high.checked_byte_add(1).is_none());
    }
    let low = unsafe { SliceStart::new_unchecked(ptr::dangling_mut::<u8>()) };
    assert!(low.checked_byte_sub(2).is_none());
    #[cfg(not(miri))]
    {
        let high_os = unsafe { OsMemPtr::new_unchecked(usize::MAX as *mut c_void) };
        assert!(high_os.checked_byte_add(1).is_none());
    }
}

#[test]
fn provenance_address_helpers_match_raw_pointer_math() {
    let bytes = [0u8; 16];
    let base = bytes.as_ptr();
    let at_9 = unsafe { base.add(9) };

    assert_eq!(ptr_exposed_addr(base), base as usize);
    assert_eq!(ptr_addr_diff(at_9, base), 9);
    assert!(ptr_is_aligned(base, 1));
    assert!(ptr_is_aligned(base, 0));
}
