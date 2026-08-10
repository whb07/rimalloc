use core::ptr;

use crate::types::SubprocPtr;

use super::{mi_subproc_prefix_typed, subprocs};

/// Lock-protected intrusive registry of all process subprocs.
///
/// The caller owns serialization with `subprocs_lock`; this module owns only
/// the `subprocs` head plus each `MiSubprocPrefix::{next, prev}` link.
pub(super) unsafe fn push_front_locked(subproc: SubprocPtr) {
    // SAFETY: registry membership rewires only the lock-protected subproc head and prev/next links.
    unsafe {
        let prefix = mi_subproc_prefix_typed(subproc);
        (*prefix).prev = ptr::null_mut();
        (*prefix).next = subprocs;
        if let Some(head) = SubprocPtr::new(subprocs) {
            (*mi_subproc_prefix_typed(head)).prev = subproc.as_ptr();
        }
        subprocs = subproc.as_ptr();
    }
}

pub(super) unsafe fn unlink_locked(subproc: SubprocPtr) {
    // SAFETY: registry membership rewires only the lock-protected subproc head and prev/next links.
    unsafe {
        let prefix = mi_subproc_prefix_typed(subproc);
        let next = SubprocPtr::new((*prefix).next);
        let prev = SubprocPtr::new((*prefix).prev);

        if let Some(next) = next {
            (*mi_subproc_prefix_typed(next)).prev = (*prefix).prev;
        }
        if let Some(prev) = prev {
            (*mi_subproc_prefix_typed(prev)).next = (*prefix).next;
        } else {
            subprocs = (*prefix).next;
        }
        (*prefix).next = ptr::null_mut();
        (*prefix).prev = ptr::null_mut();
    }
}

pub(super) unsafe fn head_locked() -> Option<SubprocPtr> {
    // SAFETY: registry membership rewires only the lock-protected subproc head and prev/next links.
    unsafe { SubprocPtr::new(subprocs) }
}

pub(super) unsafe fn next_locked(subproc: SubprocPtr) -> Option<SubprocPtr> {
    // SAFETY: registry membership rewires only the lock-protected subproc head and prev/next links.
    unsafe { SubprocPtr::new((*mi_subproc_prefix_typed(subproc)).next) }
}
