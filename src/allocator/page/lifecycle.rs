use crate::core_types::{
    AvailableFreeList, BatchValidationError, BlockPtr, DetachedRemoteList, Errno, LocalFreeList,
    LocalPage, OwnedAbandonedPage, PagePtr, PublishedRemoteHead,
};
use crate::internal::mi_block_next;
use crate::options::_mi_error_message_fmt;
use crate::types::{mi_block_t, mi_page_t};

#[cfg(test)]
use crate::internal::mi_block_set_next;
#[cfg(test)]
use core::ptr;

use super::{flags, mi_page_used_count};

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

#[inline]
fn report_remote_list_corruption(error: BatchValidationError) -> BatchValidationError {
    let message = match error {
        BatchValidationError::ExceedsUsed { .. } => {
            format_args!("corrupted meta-data in thread-free list\n")
        }
        _ => format_args!("corrupted thread-free list\n"),
    };
    _mi_error_message_fmt(Errno::EFAULT, message);
    error
}

/// # Safety
/// `detached` must be a uniquely detached chain for this local page.
#[inline]
pub(crate) unsafe fn collect_remote_for_local_page(
    page: &LocalPage,
    detached: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, BatchValidationError> {
    // SAFETY[AUDIT:page-free-list-local-detached-collect]: LocalPage and the
    // detached owner establish page identity and unique collection access.
    match unsafe { detached.into_batch_for_local(page) } {
        Ok(None) => Ok(local),
        Ok(Some(batch)) => Ok(unsafe { batch.collect_into_local_page(page, local) }),
        Err(error) => Err(report_remote_list_corruption(error)),
    }
}

/// # Safety
/// `detached` must be a uniquely detached chain for this owned abandoned page.
#[inline]
pub(crate) unsafe fn collect_remote_for_owned_abandoned_page(
    page: &OwnedAbandonedPage,
    detached: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, BatchValidationError> {
    // SAFETY[AUDIT:page-free-list-abandoned-detached-collect]: claimed
    // abandoned ownership and the detached owner exclude competing collectors.
    match unsafe { detached.into_batch_for_abandoned(page) } {
        Ok(None) => Ok(local),
        Ok(Some(batch)) => Ok(unsafe { batch.collect_into_owned_abandoned_page(page, local) }),
        Err(error) => Err(report_remote_list_corruption(error)),
    }
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
        match collect_remote_for_local_page(&page, remote, local) {
            Ok(local) => local,
            Err(_) => page_ptr.local_free(),
        }
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
    // SAFETY[AUDIT:page-free-list-collect-boundary]: typed page/list witnesses
    // preserve the raw quick-collection invariants.
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
                match local_page.detach_local_free_batch() {
                    Ok(Some(batch)) => {
                        let new_available = batch.prepend_to_available(&local_page, available);
                        page_ptr.set_free(new_available);
                        (*page).free_is_zero = false;
                        #[cfg(feature = "bench-diagnostics")]
                        crate::bench_diagnostics::page_free_collect_moved_to_free(appended);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = report_remote_list_corruption(error);
                    }
                }
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
