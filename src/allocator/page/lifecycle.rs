use crate::core_types::{
    AvailableFreeList, BlockPtr, DetachedRemoteList, Errno, LocalFreeList, LocalPage,
    OwnedAbandonedPage, PagePtr, PublishedRemoteHead,
};
use crate::internal::{mi_block_next, mi_block_set_next};
use crate::options::_mi_error_message_fmt;
use crate::types::{mi_block_t, mi_page_t};

#[cfg(test)]
use core::ptr;

use super::{flags, mi_page_capacity_count, mi_page_subtract_used_count, mi_page_used_count};

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_full(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:page-metadata-accessor-boundary]: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe { flags::mi_page_is_full(page) }
}

/// # Safety
/// `page` must point to valid page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_slice_committed(
    page: *const mi_page_t,
) -> crate::core_types::CommittedBytes {
    // SAFETY[AUDIT:page-metadata-accessor-boundary]: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe { crate::core_types::CommittedBytes::new((*page).slice_committed) }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_slice_committed(
    page: *mut mi_page_t,
    committed: crate::core_types::CommittedBytes,
) {
    // SAFETY[AUDIT:page-metadata-accessor-boundary]: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        (*page).slice_committed = committed.get();
    }
}

/// # Safety
/// `page` must point to valid page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_retire_expire(
    page: *const mi_page_t,
) -> crate::core_types::RetireExpire {
    // SAFETY[AUDIT:page-metadata-accessor-boundary]: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe { crate::core_types::RetireExpire::new((*page).retire_expire) }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_retire_expire(
    page: *mut mi_page_t,
    expire: crate::core_types::RetireExpire,
) {
    // SAFETY[AUDIT:page-metadata-accessor-boundary]: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        (*page).retire_expire = expire.get();
    }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_reset_retire_expire(page: *mut mi_page_t) {
    // SAFETY[AUDIT:page-metadata-accessor-boundary]: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        mi_page_set_retire_expire(page, crate::core_types::RetireExpire::zero());
    }
}

/// # Safety
/// `page` must point to writable page metadata with a nonzero retire countdown.
#[inline(always)]
pub(crate) unsafe fn mi_page_decrement_retire_expire(
    page: *mut mi_page_t,
) -> crate::core_types::RetireExpire {
    // SAFETY[AUDIT:page-metadata-accessor-boundary]: page metadata accessors, stats/bin helpers, and C-shaped entrypoints preserve typed counts and public ABI contracts.
    unsafe {
        let expire = mi_page_retire_expire(page).decrement();
        mi_page_set_retire_expire(page, expire);
        expire
    }
}

/// # Safety
/// `page` must be valid, and `head` must be a detached thread-free list for that page.
#[inline]
pub unsafe fn mi_page_thread_collect_to_local(page: *mut mi_page_t, head: *mut mi_block_t) {
    // SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        if head.is_null() {
            return;
        }

        let page_ptr = PagePtr::new_unchecked(page);
        let remote = DetachedRemoteList::from_raw_compat(Some(BlockPtr::new_unchecked(head)));
        let local = page_ptr.local_free();
        let local_page = crate::core_types::assume_local_page(page_ptr);
        let _ = collect_remote_for_local_page(&local_page, remote, local);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RemoteListCorruption {
    ChainTooLong,
    CountExceedsUsed,
}

/// # Safety
/// `page` must be valid, and `remote` must be a detached thread-free list for that page.
#[inline]
unsafe fn collect_detached_chain_into_local_raw(
    page_ptr: PagePtr,
    remote: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, RemoteListCorruption> {
    // SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let page = page_ptr.as_ptr();
        let Some(head_block) = remote.head() else {
            return Ok(local);
        };

        let capacity = mi_page_capacity_count(page);
        let max_count = capacity.as_usize();
        let mut count = 1usize;
        let head = head_block.as_ptr();
        let mut last = head;
        loop {
            let next = mi_block_next(page, last);
            if next.is_null() || count > max_count {
                break;
            }
            count += 1;
            last = next;
        }

        if count > max_count {
            _mi_error_message_fmt(Errno::EFAULT, format_args!("corrupted thread-free list\n"));
            return Err(RemoteListCorruption::ChainTooLong);
        }
        let used = mi_page_used_count(page);
        if count > used.as_usize() {
            _mi_error_message_fmt(
                Errno::EFAULT,
                format_args!("corrupted meta-data in thread-free list\n"),
            );
            return Err(RemoteListCorruption::CountExceedsUsed);
        }

        mi_block_set_next(page, last, local.as_raw());
        let new_local = LocalFreeList::from_head(head_block);
        page_ptr.set_local_free(new_local);

        crate::internal::mi_assert_internal!(u16::try_from(count).is_ok());
        crate::internal::mi_assert_internal!(used.get() >= count as u16);
        mi_page_subtract_used_count(page, count as u16);
        Ok(new_local)
    }
}

/// # Safety
/// `detached` must be a uniquely detached chain for this local page.
#[inline]
pub(crate) unsafe fn collect_remote_for_local_page(
    page: &LocalPage,
    detached: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, RemoteListCorruption> {
    // SAFETY[AUDIT:page-free-list-local-detached-collect]: `LocalPage`
    // establishes current-thread ownership for the raw collector.
    unsafe { collect_detached_chain_into_local_raw(page.page(), detached, local) }
}

/// # Safety
/// `detached` must be a uniquely detached chain for this owned abandoned page.
#[inline]
pub(crate) unsafe fn collect_remote_for_owned_abandoned_page(
    page: &OwnedAbandonedPage,
    detached: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, RemoteListCorruption> {
    // SAFETY[AUDIT:page-free-list-abandoned-detached-collect]: owned abandoned
    // state establishes exclusive collection rights for the raw collector.
    unsafe { collect_detached_chain_into_local_raw(page.page(), detached, local) }
}

/// # Safety
/// `page` must point to page metadata owned by the current allocator state.
#[inline]
pub unsafe fn mi_page_thread_free_collect(page: *mut mi_page_t) {
    // SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let page_ptr = PagePtr::new_unchecked(page);
        let local_page = crate::core_types::assume_local_page(page_ptr);
        let atomic_tf = page_ptr.thread_free_cell();
        let remote = atomic_tf.detach_head();
        if remote.is_empty() {
            return;
        }
        let _ = mi_page_thread_free_collect_typed(local_page, remote);
    }
}

/// # Safety
/// `page` must point to page metadata owned by the current allocator state.
#[inline]
pub(crate) unsafe fn mi_page_thread_free_collect_typed(
    page: LocalPage,
    remote: DetachedRemoteList,
) -> LocalFreeList {
    // SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let page_ptr = page.page();
        let local = page_ptr.local_free();
        collect_remote_for_local_page(&page, remote, local).unwrap_or(local)
    }
}

/// # Safety
/// `page` must point to page metadata owned by the current allocator state.
#[inline]
pub unsafe fn mi_page_free_quick_collect(page: *mut mi_page_t) -> bool {
    // SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let page_ptr = PagePtr::new_unchecked(page);
        let local_page = crate::core_types::assume_local_page(page_ptr);
        let available = page_ptr.free();
        let local = page_ptr.local_free();
        mi_page_free_quick_collect_typed(local_page, available, local)
    }
}

/// # Safety
/// `page` must point to page metadata owned by the current allocator state.
#[inline]
pub(crate) unsafe fn mi_page_free_quick_collect_typed(
    page: LocalPage,
    available: AvailableFreeList,
    local: LocalFreeList,
) -> bool {
    // SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe { mi_page_free_quick_collect_raw(page.page(), available, local) }
}

#[inline(always)]
unsafe fn mi_page_free_quick_collect_raw(
    page_ptr: PagePtr,
    available: AvailableFreeList,
    local: LocalFreeList,
) -> bool {
    unsafe {
        if !available.is_empty() {
            return true;
        }
        if local.is_empty() {
            return false;
        }
        let page = page_ptr.as_ptr();
        let new_available = local.into_available();
        let new_local = LocalFreeList::empty();
        page_ptr.set_free(new_available);
        page_ptr.set_local_free(new_local);
        (*page).free_is_zero = false;
        true
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
// SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_free_collect(page: *mut mi_page_t, force: bool) {
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        #[cfg(feature = "bench-diagnostics")]
        crate::bench_diagnostics::page_free_collect_call(force);

        mi_page_thread_free_collect(page);

        let page_ptr = PagePtr::new_unchecked(page);
        let local_page = crate::core_types::assume_local_page(page_ptr);
        let local = page_ptr.local_free();
        if !local.is_empty() {
            let available = page_ptr.free();
            if available.is_empty() || force {
                #[cfg(feature = "bench-diagnostics")]
                let appended = !available.is_empty();
                let new_available = local.prepend_to_available(local_page, available);
                page_ptr.set_free(new_available);
                page_ptr.set_local_free(LocalFreeList::empty());
                (*page).free_is_zero = false;
                #[cfg(feature = "bench-diagnostics")]
                crate::bench_diagnostics::page_free_collect_moved_to_free(appended);
            } else {
                #[cfg(feature = "bench-diagnostics")]
                crate::bench_diagnostics::page_free_collect_noop_local_blocked();
            }
        } else {
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::page_free_collect_local_empty();
        }

        crate::internal::mi_assert_internal!(!force || page_ptr.local_free().is_empty());
    }
}

/// # Safety
/// `page` must be valid, and `head` must be null or a detached block list belonging to `page`.
// SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_free_collect_partly(page: *mut mi_page_t, head: *mut mi_block_t) {
    unsafe {
        #[cfg(feature = "bench-diagnostics")]
        crate::bench_diagnostics::page_free_collect_partly_call(head.is_null());
        if head.is_null() {
            return;
        }
        let page_ptr = PagePtr::new_unchecked(page);
        let owned_page = crate::core_types::assume_owned_abandoned_page(page_ptr);
        let remote_head = PublishedRemoteHead::from_raw_compat(BlockPtr::new_unchecked(head));
        _mi_page_free_collect_partly_typed(&owned_page, remote_head);
    }
}

/// # Safety
/// `page` must be valid, and `remote_head` must remain published for `page`.
#[inline]
pub(crate) unsafe fn _mi_page_free_collect_partly_typed(
    owned_page: &OwnedAbandonedPage,
    remote_head: PublishedRemoteHead,
) {
    // SAFETY[AUDIT:page-free-list-collect-boundary]: page-local free-list collection, extension, search, and refill preserve block ownership, capacity, commit, and availability invariants.
    unsafe {
        let page_ptr = owned_page.page();
        let page = page_ptr.as_ptr();
        let head_block = remote_head.head();
        let head = head_block.as_ptr();
        let (remote_head, remote_tail) = remote_head.detach_tail(owned_page);
        if let Some(remote_tail) = remote_tail {
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::page_free_collect_partly_detached();
            let local = page_ptr.local_free();
            let _ = collect_remote_for_owned_abandoned_page(owned_page, remote_tail, local);
            if !page_ptr.local_free().is_empty() && page_ptr.free().is_empty() {
                let available = page_ptr.free();
                let local = page_ptr.local_free();
                let _ = mi_page_free_quick_collect_raw(page_ptr, available, local);
            }
        }
        remote_head.leave_published();
        if mi_page_used_count(page).get() == 1 {
            let atomic_tf = page_ptr.thread_free_cell();
            crate::internal::mi_assert_internal!(atomic_tf.load_relaxed().head_raw() == head);
            crate::internal::mi_assert_internal!(mi_block_next(page, head).is_null());
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::page_free_collect_partly_final_collect();
            _mi_page_free_collect(page, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_prepend_to_available_splices_tail_to_existing_available_list() {
        let mut blocks = [ptr::null_mut::<mi_block_t>(); 16];
        // SAFETY[AUDIT:page-free-list-collect-boundary]: this allocates real
        // allocator-owned blocks so encoded free-list link checks see
        // registered same-page pointers while exercising only the typed splice.
        unsafe {
            for block in &mut blocks {
                *block = crate::mimalloc::mi_malloc(core::mem::size_of::<mi_block_t>()).cast();
                assert!(!(*block).is_null());
            }

            let mut chosen = None;
            'outer: for i in 0..blocks.len() {
                let page = crate::internal::_mi_ptr_page(blocks[i].cast());
                if page.is_null() {
                    continue;
                }
                for j in (i + 1)..blocks.len() {
                    if crate::internal::_mi_ptr_page(blocks[j].cast()) != page {
                        continue;
                    }
                    for k in (j + 1)..blocks.len() {
                        if crate::internal::_mi_ptr_page(blocks[k].cast()) == page {
                            chosen = Some((page, blocks[i], blocks[j], blocks[k]));
                            break 'outer;
                        }
                    }
                }
            }
            let Some((raw_page, local_head, local_tail, available_head)) = chosen else {
                for block in blocks {
                    crate::mimalloc::mi_free(block.cast());
                }
                panic!("expected three same-page small allocations for free-list splice fixture");
            };

            let page_ptr = PagePtr::new_unchecked(raw_page);
            let local_page = crate::core_types::assume_local_page(page_ptr);
            mi_block_set_next(raw_page, local_head, local_tail);
            mi_block_set_next(raw_page, local_tail, ptr::null());
            mi_block_set_next(raw_page, available_head, ptr::null());

            let local = LocalFreeList::from_raw(local_head);
            let available = AvailableFreeList::from_raw(available_head);
            let spliced = local.prepend_to_available(local_page, available);

            assert_eq!(spliced.as_raw(), local_head);
            assert_eq!(mi_block_next(raw_page, local_head), local_tail);
            assert_eq!(mi_block_next(raw_page, local_tail), available_head);
            assert!(mi_block_next(raw_page, available_head).is_null());

            for block in blocks {
                crate::mimalloc::mi_free(block.cast());
            }
        }
    }

    #[test]
    fn published_head_detaches_only_its_tail() {
        let mut blocks = [ptr::null_mut::<mi_block_t>(); 16];
        // SAFETY[AUDIT:page-remote-list-tail-fixture]: real allocator-owned
        // blocks keep encoded-link validation within one registered page.
        unsafe {
            for block in &mut blocks {
                *block = crate::mimalloc::mi_malloc(core::mem::size_of::<mi_block_t>()).cast();
                assert!(!block.is_null());
            }
            let mut chosen = None;
            'outer: for i in 0..blocks.len() {
                let page = crate::internal::_mi_ptr_page(blocks[i].cast());
                for j in (i + 1)..blocks.len() {
                    if crate::internal::_mi_ptr_page(blocks[j].cast()) == page {
                        chosen = Some((page, blocks[i], blocks[j]));
                        break 'outer;
                    }
                }
            }
            let (page, head, tail) = chosen.expect("expected two same-page allocations");
            let page_ptr = PagePtr::new_unchecked(page);
            let head_ptr = BlockPtr::new_unchecked(head);
            let tail_ptr = BlockPtr::new_unchecked(tail);
            mi_block_set_next(page, head, tail);
            mi_block_set_next(page, tail, ptr::null_mut());

            let owned = crate::core_types::assume_owned_abandoned_page(page_ptr);
            let published = PublishedRemoteHead::from_raw_compat(head_ptr);
            let (published, detached) = published.detach_tail(&owned);
            assert_eq!(published.head().as_ptr(), head);
            assert!(mi_block_next(page, head).is_null());
            assert_eq!(
                detached.expect("tail").head().map(BlockPtr::as_ptr),
                Some(tail_ptr.as_ptr())
            );
            published.leave_published();
            for block in blocks {
                crate::mimalloc::mi_free(block.cast());
            }
        }
    }
}
