use core::ptr::{self, NonNull};

use crate::types::{MiLockGuard, MiLockPtr, SubprocPtr, mi_subproc_t};

use super::{mi_subproc_prefix_typed, subprocs, subprocs_lock};

/// Exclusive access to the process-global intrusive subproc registry.
pub(super) struct SubprocRegistryGuard<'a> {
    _lock: MiLockGuard<'a>,
    head: NonNull<*mut mi_subproc_t>,
}

/// A live subproc removed exactly once from the process registry.
#[must_use = "detached subproc ownership must be consumed by destruction"]
pub(super) struct DetachedSubproc(SubprocPtr);

impl DetachedSubproc {
    #[inline(always)]
    pub(super) fn ptr(&self) -> SubprocPtr {
        self.0
    }
}

impl SubprocRegistryGuard<'static> {
    /// # Safety
    /// The process-global subproc lock must be initialized and remain live.
    pub(super) unsafe fn acquire() -> Self {
        // SAFETY: required by this constructor's contract; the guard releases
        // the same process-global lock on drop.
        let lock = MiLockPtr::from_nonnull(unsafe {
            NonNull::new_unchecked(ptr::addr_of_mut!(subprocs_lock))
        });
        Self {
            // SAFETY: the process-global lock is initialized by `main_init`.
            _lock: unsafe { MiLockGuard::acquire(lock) },
            // SAFETY: a static variable has a stable non-null address.
            head: unsafe { NonNull::new_unchecked(ptr::addr_of_mut!(subprocs)) },
        }
    }

    /// # Safety
    /// `subproc` must be initialized, live, and not currently linked.
    pub(super) unsafe fn push_front(&self, subproc: SubprocPtr) {
        // SAFETY: this guard exclusively owns the registry and the caller
        // provides an initialized unlinked node.
        unsafe { push_front(self.head.as_ptr(), subproc) };
    }

    /// Looks up `candidate` by address without dereferencing it, then unlinks
    /// the matching live registry node exactly once.
    pub(super) fn detach(&self, candidate: *mut mi_subproc_t) -> Option<DetachedSubproc> {
        // SAFETY: this guard exclusively owns the initialized registry. The
        // candidate is compared as an address and dereferenced only after it
        // equals a node reached from the registry head.
        unsafe { detach_from(self.head.as_ptr(), candidate) }
    }

    pub(super) fn detach_first_except(
        &self,
        excluded: *mut mi_subproc_t,
    ) -> Option<DetachedSubproc> {
        // SAFETY: this guard exclusively owns the initialized registry.
        unsafe { detach_first_except_from(self.head.as_ptr(), excluded) }
    }
}

unsafe fn push_front(head: *mut *mut mi_subproc_t, subproc: SubprocPtr) {
    // SAFETY: caller owns the registry and provides an initialized unlinked node.
    unsafe {
        let prefix = mi_subproc_prefix_typed(subproc);
        (*prefix).prev = ptr::null_mut();
        (*prefix).next = *head;
        if let Some(old_head) = SubprocPtr::new(*head) {
            (*mi_subproc_prefix_typed(old_head)).prev = subproc.as_ptr();
        }
        *head = subproc.as_ptr();
    }
}

unsafe fn detach_from(
    head: *mut *mut mi_subproc_t,
    candidate: *mut mi_subproc_t,
) -> Option<DetachedSubproc> {
    // SAFETY: caller provides exclusive access to the initialized head word.
    let mut current = SubprocPtr::new(unsafe { *head });
    while let Some(linked) = current {
        if ptr::eq(linked.as_ptr(), candidate) {
            // SAFETY: `linked` was reached from this exclusively owned registry.
            unsafe { unlink(head, linked) };
            return Some(DetachedSubproc(linked));
        }
        // SAFETY: `linked` was reached from this exclusively owned registry.
        current = SubprocPtr::new(unsafe { (*mi_subproc_prefix_typed(linked)).next });
    }
    None
}

unsafe fn detach_first_except_from(
    head: *mut *mut mi_subproc_t,
    excluded: *mut mi_subproc_t,
) -> Option<DetachedSubproc> {
    // SAFETY: caller provides exclusive access to the initialized head word.
    let mut current = SubprocPtr::new(unsafe { *head });
    while let Some(linked) = current {
        if !ptr::eq(linked.as_ptr(), excluded) {
            // SAFETY: `linked` was reached from this exclusively owned registry.
            unsafe { unlink(head, linked) };
            return Some(DetachedSubproc(linked));
        }
        // SAFETY: `linked` was reached from this exclusively owned registry.
        current = SubprocPtr::new(unsafe { (*mi_subproc_prefix_typed(linked)).next });
    }
    None
}

unsafe fn unlink(head: *mut *mut mi_subproc_t, subproc: SubprocPtr) {
    // SAFETY: caller owns the registry and `subproc` is a node reached from it.
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
            *head = (*prefix).next;
        }
        (*prefix).next = ptr::null_mut();
        (*prefix).prev = ptr::null_mut();
    }
}

#[cfg(test)]
mod tests {
    use core::ptr;

    use super::*;
    use crate::internal::memid::mi_memid_t;
    use crate::types::SubprocSeq;

    fn node(seq: usize) -> super::super::MiSubprocPrefix {
        super::super::MiSubprocPrefix::new_with_memid(SubprocSeq::new(seq), mi_memid_t::default())
    }

    unsafe fn node_ptr(node: &mut super::super::MiSubprocPrefix) -> SubprocPtr {
        // SAFETY: the caller keeps this stack node live for the whole list operation.
        unsafe { SubprocPtr::new_unchecked(ptr::from_mut(node).cast::<mi_subproc_t>()) }
    }

    unsafe fn links(node: SubprocPtr) -> (*mut mi_subproc_t, *mut mi_subproc_t) {
        // SAFETY: the caller keeps the initialized stack node live.
        let prefix = unsafe { mi_subproc_prefix_typed(node) };
        // SAFETY: the node's link fields are initialized and readable.
        unsafe {
            (
                ptr::read(ptr::addr_of!((*prefix).next)),
                ptr::read(ptr::addr_of!((*prefix).prev)),
            )
        }
    }

    #[test]
    fn detach_rejects_forged_stale_and_double_ids_without_dereference() {
        let mut first = node(1);
        let mut head = ptr::null_mut();
        // SAFETY: the stack node is initialized, live, and initially unlinked.
        let first_ptr = unsafe { node_ptr(&mut first) };
        // SAFETY: this test exclusively owns the local head and node.
        unsafe { push_front(ptr::addr_of_mut!(head), first_ptr) };

        let forged = NonNull::<mi_subproc_t>::dangling().as_ptr();
        // SAFETY: the local registry is initialized and exclusively owned;
        // `forged` is compared only by address.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), forged) }.is_none());
        // SAFETY: the local registry is initialized and exclusively owned.
        let detached = unsafe { detach_from(ptr::addr_of_mut!(head), first_ptr.as_ptr()) };
        assert_eq!(
            detached.map(|node| node.ptr().as_ptr()),
            Some(first_ptr.as_ptr())
        );
        // SAFETY: the now-empty local registry is initialized and exclusively owned.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), first_ptr.as_ptr()) }.is_none());
        assert!(head.is_null());
    }

    #[test]
    fn detach_preserves_head_middle_and_tail_topology() {
        let mut first = node(1);
        let mut middle = node(2);
        let mut last = node(3);
        let mut head = ptr::null_mut();
        // SAFETY: the stack nodes remain live and initially unlinked.
        let (first_ptr, middle_ptr, last_ptr) = unsafe {
            (
                node_ptr(&mut first),
                node_ptr(&mut middle),
                node_ptr(&mut last),
            )
        };
        // SAFETY: this test exclusively owns the local head and nodes.
        unsafe {
            push_front(ptr::addr_of_mut!(head), first_ptr);
            push_front(ptr::addr_of_mut!(head), middle_ptr);
            push_front(ptr::addr_of_mut!(head), last_ptr);
        }

        // SAFETY: the local registry is initialized and exclusively owned.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), middle_ptr.as_ptr()) }.is_some());
        assert_eq!(head, last_ptr.as_ptr());
        // SAFETY: all stack nodes remain initialized and live.
        let (last_next, last_prev) = unsafe { links(last_ptr) };
        // SAFETY: all stack nodes remain initialized and live.
        let (_, first_prev) = unsafe { links(first_ptr) };
        // SAFETY: all stack nodes remain initialized and live.
        let (middle_next, middle_prev) = unsafe { links(middle_ptr) };
        assert_eq!(last_next, first_ptr.as_ptr());
        assert!(last_prev.is_null());
        assert_eq!(first_prev, last_ptr.as_ptr());
        assert!(middle_next.is_null());
        assert!(middle_prev.is_null());

        // SAFETY: the local registry is initialized and exclusively owned.
        let detached =
            unsafe { detach_first_except_from(ptr::addr_of_mut!(head), last_ptr.as_ptr()) };
        assert_eq!(
            detached.map(|node| node.ptr().as_ptr()),
            Some(first_ptr.as_ptr())
        );
        assert_eq!(head, last_ptr.as_ptr());
        // SAFETY: the stack node remains initialized and live.
        let (last_next, last_prev) = unsafe { links(last_ptr) };
        assert!(last_next.is_null());
        assert!(last_prev.is_null());

        // SAFETY: the local registry is initialized and exclusively owned.
        assert!(unsafe { detach_from(ptr::addr_of_mut!(head), last_ptr.as_ptr()) }.is_some());
        assert!(head.is_null());
    }
}
