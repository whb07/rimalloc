use crate::allocator::options::_mi_error_message_fmt;
use crate::core_types::{
    AvailableFreeList, BatchValidationError, DetachedRemoteList, Errno, LocalFreeList, LocalPage,
    OwnedAbandonedPage, PagePtr, PublishedRemoteHead,
};
use crate::internal::mi_block_next;
use crate::types::mi_page_t;

#[cfg(test)]
use crate::core_types::BlockPtr;
#[cfg(test)]
use crate::internal::mi_block_set_next;
#[cfg(test)]
use crate::types::mi_block_t;
#[cfg(test)]
use core::ptr;

use super::mi_page_used_count;

/// # Safety
/// `page` must point to valid page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_slice_committed(
    page: *const mi_page_t,
) -> crate::core_types::CommittedBytes {
    unsafe { crate::core_types::CommittedBytes::new((*page).slice_committed) }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_slice_committed(
    page: *mut mi_page_t,
    committed: crate::core_types::CommittedBytes,
) {
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
    unsafe { crate::core_types::RetireExpire::new((*page).retire_expire) }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_set_retire_expire(
    page: *mut mi_page_t,
    expire: crate::core_types::RetireExpire,
) {
    unsafe {
        (*page).retire_expire = expire.get();
    }
}

/// # Safety
/// `page` must point to writable page metadata.
#[inline(always)]
pub(crate) unsafe fn mi_page_reset_retire_expire(page: *mut mi_page_t) {
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
    unsafe {
        let expire = mi_page_retire_expire(page).decrement();
        mi_page_set_retire_expire(page, expire);
        expire
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

#[inline]
fn report_local_list_corruption(error: BatchValidationError) -> BatchValidationError {
    let message = match error {
        BatchValidationError::ExceedsUsed { .. } => {
            format_args!("corrupted meta-data in local free list\n")
        }
        _ => format_args!("corrupted local free list\n"),
    };
    _mi_error_message_fmt(Errno::EFAULT, message);
    error
}

enum PageCollectOwner<'a> {
    Local(&'a LocalPage),
    Abandoned(&'a OwnedAbandonedPage),
}

impl PageCollectOwner<'_> {
    #[inline(always)]
    fn page(&self) -> PagePtr {
        match self {
            Self::Local(owner) => owner.page(),
            Self::Abandoned(owner) => owner.page(),
        }
    }

    #[inline]
    unsafe fn collect_remote(
        &self,
        detached: DetachedRemoteList,
        local: LocalFreeList,
    ) -> Result<LocalFreeList, BatchValidationError> {
        match self {
            Self::Local(owner) => unsafe { collect_remote_for_local_page(owner, detached, local) },
            Self::Abandoned(owner) => unsafe {
                collect_remote_for_owned_abandoned_page(owner, detached, local)
            },
        }
    }

    #[inline]
    unsafe fn detach_local_free_batch(
        &self,
    ) -> Result<Option<crate::core_types::LocalFreeBatch>, BatchValidationError> {
        match self {
            Self::Local(owner) => unsafe { owner.detach_local_free_batch() },
            Self::Abandoned(owner) => unsafe { owner.detach_local_free_batch() },
        }
    }

    #[inline]
    unsafe fn prepend_to_available(
        &self,
        batch: crate::core_types::LocalFreeBatch,
        available: AvailableFreeList,
    ) -> AvailableFreeList {
        match self {
            Self::Local(owner) => unsafe { batch.prepend_to_available(owner, available) },
            Self::Abandoned(owner) => unsafe {
                batch.prepend_to_owned_abandoned_page(owner, available)
            },
        }
    }

    #[inline(always)]
    unsafe fn transfer_local_to_available(&self, local: LocalFreeList) {
        match self {
            // SAFETY: the concrete owner and caller-proven list refer to the
            // same exclusively accessed page.
            Self::Local(owner) => unsafe { owner.transfer_local_to_available(local) },
            // SAFETY: the concrete owner and caller-proven list refer to the
            // same claimed abandoned page.
            Self::Abandoned(owner) => unsafe { owner.transfer_local_to_available(local) },
        }
    }

    #[inline(always)]
    unsafe fn publish_recycled_available(&self, available: AvailableFreeList) {
        match self {
            // SAFETY: the concrete owner and caller-proven list refer to the
            // same exclusively accessed page.
            Self::Local(owner) => unsafe { owner.publish_recycled_available(available) },
            // SAFETY: the concrete owner and caller-proven list refer to the
            // same claimed abandoned page.
            Self::Abandoned(owner) => unsafe { owner.publish_recycled_available(available) },
        }
    }
}

/// # Safety
/// `detached` must be a uniquely detached chain for this local page.
#[inline]
pub(crate) unsafe fn collect_remote_for_local_page(
    page: &LocalPage,
    detached: DetachedRemoteList,
    local: LocalFreeList,
) -> Result<LocalFreeList, BatchValidationError> {
    match unsafe { detached.into_batch_for_local(page) } {
        Ok(None) => Ok(local),
        Ok(Some(batch)) => {
            // Native fast remote frees publish without entering the Rust free path.
            super::mi_process_mark_multi_threaded();
            Ok(unsafe { batch.collect_into_local_page(page, local) })
        }
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
    match unsafe { detached.into_batch_for_abandoned(page) } {
        Ok(None) => Ok(local),
        Ok(Some(batch)) => Ok(unsafe { batch.collect_into_owned_abandoned_page(page, local) }),
        Err(error) => Err(report_remote_list_corruption(error)),
    }
}

#[inline]
unsafe fn collect_page_free_lists(owner: PageCollectOwner<'_>, force: bool) {
    let page_ptr = owner.page();
    let remote = unsafe { page_ptr.thread_free_cell().detach_head() };
    if !remote.is_empty() {
        let local = unsafe { page_ptr.local_free() };
        let _ = unsafe { owner.collect_remote(remote, local) };
    }

    let local = unsafe { page_ptr.local_free() };
    if !local.is_empty() {
        let available = unsafe { page_ptr.free() };
        if available.is_empty() {
            unsafe { owner.transfer_local_to_available(local) };
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_free_collect_moved_to_free(false);
        } else if force {
            match unsafe { owner.detach_local_free_batch() } {
                Ok(Some(batch)) => {
                    let new_available = unsafe { owner.prepend_to_available(batch, available) };
                    // SAFETY: the splice result is now the owner's unique
                    // recycled available-list head.
                    unsafe { owner.publish_recycled_available(new_available) };
                    #[cfg(feature = "bench-diagnostics")]
                    crate::allocator::bench_diagnostics::page_free_collect_moved_to_free(true);
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = report_local_list_corruption(error);
                }
            }
        } else {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_free_collect_noop_local_blocked();
        }
    } else {
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::page_free_collect_local_empty();
    }

    crate::internal::mi_assert_internal!(!force || unsafe { page_ptr.local_free() }.is_empty());
}

/// Collect free lists for a page owned by the current thread heap.
///
/// # Safety
/// `page` must remain the unique local collection owner for the call.
#[inline]
pub(crate) unsafe fn collect_page_free_lists_local(page: &LocalPage, force: bool) {
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::page_free_collect_local_call(force);
    unsafe { collect_page_free_lists(PageCollectOwner::Local(page), force) };
}

/// Collect free lists for a page whose abandoned ownership bit is claimed.
///
/// # Safety
/// `page` must remain the unique abandoned collection owner for the call.
#[inline]
pub(crate) unsafe fn collect_page_free_lists_abandoned(page: &OwnedAbandonedPage, force: bool) {
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::page_free_collect_abandoned_call(force);
    unsafe { collect_page_free_lists(PageCollectOwner::Abandoned(page), force) };
}

/// # Safety
/// `page` must point to page metadata owned by the current thread heap. Owned
/// abandoned pages must use `collect_page_free_lists_abandoned` instead.
#[inline(never)]
pub unsafe extern "C" fn _mi_page_free_collect(page: *mut mi_page_t, force: bool) {
    unsafe {
        crate::internal::mi_assert_internal!(!page.is_null());
        let page_ptr = PagePtr::new_unchecked(page);
        let local_page = crate::allocator::raw_bridge::assume_local_page(page_ptr);
        collect_page_free_lists_local(&local_page, force);
    }
}

/// # Safety
/// `owned_page` must retain the unique claim on a live abandoned page for this
/// operation. `remote_head` must be the exact non-null head still published in
/// that page's atomic thread-free word. The page backing, encoded-link keys,
/// used count, and available/local free-list fields must remain live and
/// structurally valid; the claimed ownership must exclude another collector,
/// owner transition, relink, or backing release while remote frees may publish
/// only through the atomic thread-free protocol. Any diagnostic callback and
/// opaque argument must remain valid, must not incompatibly reenter page/list
/// mutation, and must not unwind across its C ABI invocation.
///
/// The operation first detaches and collects only the published head's tail,
/// leaves the head published, and only then observes `used == 1` to decide
/// whether to collect the final one-block remote list.
#[inline]
pub(crate) unsafe fn _mi_page_free_collect_partly_typed(
    owned_page: &OwnedAbandonedPage,
    remote_head: PublishedRemoteHead,
) {
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::page_free_collect_partly_call(false);
    let page_ptr = owned_page.page();
    let page = page_ptr.as_ptr();
    let head = remote_head.head().as_ptr();

    // SAFETY: the claimed abandoned owner and exact published head exclude a
    // competing collector. Detaching severs only the head's tail; the detached
    // chain and all page-local list fields remain exclusively mutable here.
    // List snapshots are intentionally repeated to preserve publication order.
    let remote_head = unsafe {
        let (remote_head, remote_tail) = remote_head.detach_tail(owned_page);
        if let Some(remote_tail) = remote_tail {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_free_collect_partly_detached();
            let local = page_ptr.local_free();
            let _ = collect_remote_for_owned_abandoned_page(owned_page, remote_tail, local);
            if !page_ptr.local_free().is_empty() && page_ptr.free().is_empty() {
                let local = page_ptr.local_free();
                owned_page.transfer_local_to_available(local);
            }
        }
        remote_head
    };

    remote_head.leave_published();

    // SAFETY: the page and its claimed ownership remain live after leaving the
    // publication witness. The used-count check intentionally follows that
    // transition; when one block remains, the atomic head and its null link
    // prove the final one-block list before the terminal full collection.
    unsafe {
        if mi_page_used_count(page).get() == 1 {
            let atomic_tf = page_ptr.thread_free_cell();
            crate::internal::mi_assert_internal!(atomic_tf.load_relaxed().head_raw() == head);
            crate::internal::mi_assert_internal!(mi_block_next(page, head).is_null());
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_free_collect_partly_final_collect();
            collect_page_free_lists_abandoned(owned_page, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_head_detaches_only_its_tail() {
        let mut blocks = [ptr::null_mut::<mi_block_t>(); 16];
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

            let owned = crate::allocator::raw_bridge::assume_owned_abandoned_page(page_ptr);
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
